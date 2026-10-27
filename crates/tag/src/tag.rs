//! `mise run tag`: sets the version, commits it, tags it and pushes. It asks before each step,
//! Enter taking the default:
//!
//! 1. on main, with nothing uncommitted or untracked and nothing on origin's main it lacks: the
//!    new version (the next patch by default; the current one when it was never tagged, as for
//!    the first release), written to the root Cargo.toml and Cargo.lock;
//! 2. the commit, of those two files only (`[release] kuleshov <version>` by default; the hooks
//!    run);
//! 3. the annotated tag `v<version>`;
//! 4. `git push --atomic origin main <tag>`, or how to push later and how to undo it here.
//!
//! Plain Git does it all, with whatever credentials it already pushes with: there is no GitHub
//! release and no crates.io upload (the crates are `publish = false`), so no token. Nothing
//! changes before the commit; after it, the messages say what was done and how to undo it.

use std::{
    fs,
    io::{BufRead, Write},
    path::Path,
    process::{Command, Stdio},
};

use crate::{
    manifest::{lock_with_version, with_workspace_version, workspace_version},
    version::Version,
};

/// Why the tool stopped, with the exit code it ends with.
struct Stop {
    reason: String,
    code: u8,
}

impl From<std::io::Error> for Stop {
    fn from(error: std::io::Error) -> Self {
        Self {
            reason: error.to_string(),
            code: 1,
        }
    }
}

/// Stops with exit code 1.
fn stop<T>(reason: impl Into<String>) -> Result<T, Stop> {
    Err(Stop {
        reason: reason.into(),
        code: 1,
    })
}

/// Runs `mise run tag` in the repository at `root`, reading the answers from `input` (one line
/// each) and writing the questions and messages to `out`; Git's and Cargo's own output goes to
/// the process's. Returns the exit code: 0 when tagged and pushed, or when the push is left for
/// later; 1 when it stopped.
pub fn run(root: &Path, input: &mut impl BufRead, out: &mut impl Write) -> u8 {
    let mut session = Session {
        root,
        input,
        out,
        done: Vec::new(),
        undo: Vec::new(),
    };
    match session.tag() {
        Ok(()) => 0,
        Err(stop) => {
            // Nothing else can be said when the messages cannot be written.
            let _ = session.report(&stop.reason);
            stop.code
        }
    }
}

/// What the checks found, for the steps after them.
struct State {
    /// The root Cargo.toml's text.
    cargo_toml: String,
    current: Version,
    /// Whether the current version has its tag, here or on origin.
    current_tagged: bool,
    /// Where origin's main is, if it has one.
    origin_main: Option<String>,
    origin_tags: Vec<String>,
}

struct Session<'a, I, O> {
    root: &'a Path,
    input: &'a mut I,
    out: &'a mut O,
    /// What has been done, and the commands that undo it here (printed, never run).
    done: Vec<String>,
    undo: Vec<String>,
}

impl<I: BufRead, O: Write> Session<'_, I, O> {
    fn tag(&mut self) -> Result<(), Stop> {
        let state = self.check()?;
        let version = self.choose_version(&state)?;
        let tag = version.tag();
        let bump = version > state.current;
        if !bump {
            self.say(&format!(
                "{version} is Cargo.toml's already: no commit, the tag goes on HEAD."
            ))?;
        }
        let default_message = format!("[release] kuleshov {version}");
        let mut message = default_message.clone();
        if bump {
            self.step("The commit")?;
            let answer = self.ask(&format!("Commit message [{default_message}]: "))?;
            if !answer.is_empty() {
                message = answer;
            }
        }
        self.step("The tag")?;
        let target = if bump { "the new commit" } else { "HEAD" };
        if !is_yes(&self.ask(&format!("Tag {target} {tag}? [Y/n] "))?, true) {
            return stop("Stopped: nothing changed.");
        }
        if bump {
            self.set_version(&state.cargo_toml, &version)?;
            self.commit(message, &default_message, &version)?;
        }
        self.step(&format!("The tag {tag}"))?;
        self.run(
            "git",
            &[
                "tag",
                "--annotate",
                &tag,
                "--message",
                &format!("kuleshov {version}"),
            ],
        )?;
        self.done
            .push(format!("tagged HEAD {tag}, annotated, here only"));
        self.undo.insert(0, format!("git tag -d {tag}"));
        self.push(&tag, state.origin_main.as_deref())
    }

    /// Checks the repository is ready for a version, and reads what the steps need.
    fn check(&mut self) -> Result<State, Stop> {
        self.step("Before starting")?;
        let status = self.capture("git", &["status", "--porcelain", "--untracked-files=all"])?;
        if !status.trim().is_empty() {
            return stop(format!(
                "Uncommitted changes or untracked files: commit those first, each with its own prefix.\n{}",
                status.trim_end()
            ));
        }
        match self.try_capture("git", &["symbolic-ref", "--quiet", "--short", "HEAD"])? {
            Some(branch) if branch.trim() == "main" => {}
            Some(branch) => {
                return stop(format!(
                    "HEAD is on {}: a version is set on main.",
                    branch.trim()
                ));
            }
            None => return stop("HEAD is detached: a version is set on main."),
        }
        let Some(head) = self.commit_of("HEAD")? else {
            return stop("HEAD has no commit.");
        };
        let origin_main = self.remote_main()?;
        if let Some(origin_main) = &origin_main
            && !self.is_ancestor(origin_main, &head)?
        {
            return stop(format!(
                "origin's main ({}) has commits this main lacks: pull them first.",
                short(origin_main)
            ));
        }
        let origin_tags = self.remote_tags()?;
        let cargo_toml = fs::read_to_string(self.root.join("Cargo.toml"))?;
        let Some(current_text) = workspace_version(&cargo_toml) else {
            return stop("No version in Cargo.toml's [workspace.package].");
        };
        let Some(current) = Version::parse(current_text) else {
            return stop(format!("Cargo.toml's version {current_text} is not x.y.z."));
        };
        let current_tagged = self.tag_taken(&current.tag(), &origin_tags)?;
        let never = if current_tagged {
            String::new()
        } else {
            format!(", never tagged (no {})", current.tag())
        };
        self.say(&format!("\nThe version is {current}{never}."))?;
        Ok(State {
            cargo_toml,
            current,
            current_tagged,
            origin_main,
            origin_tags,
        })
    }

    /// Asks for the new version until it is one after the current and not tagged yet.
    fn choose_version(&mut self, state: &State) -> Result<Version, Stop> {
        self.step("The version")?;
        let current = &state.current;
        let suggested = if state.current_tagged {
            current.next_patch()
        } else {
            current.clone()
        };
        loop {
            let answer = self.ask(&format!("New version [{suggested}]: "))?;
            let typed = if answer.is_empty() {
                suggested.to_string()
            } else {
                answer
            };
            let Some(version) = Version::parse(&typed) else {
                self.say(&format!(
                    "{typed} is not a version: x.y.z, or x.y.z-alpha.1 for a pre-release."
                ))?;
                continue;
            };
            if version < *current || (version == *current && state.current_tagged) {
                self.say(&format!("{typed} is not after {current}."))?;
            } else if self.tag_taken(&version.tag(), &state.origin_tags)? {
                self.say(&format!(
                    "{} exists already, here or on origin.",
                    version.tag()
                ))?;
            } else {
                return Ok(version);
            }
        }
    }

    /// Writes `version` to Cargo.toml (`cargo_toml` is its text) and Cargo.lock, checking that
    /// nothing else changes; on a failure it puts them back.
    fn set_version(&mut self, cargo_toml: &str, version: &Version) -> Result<(), Stop> {
        self.step(&format!("{version} in Cargo.toml and Cargo.lock"))?;
        let lock_path = self.root.join("Cargo.lock");
        let lock_before = fs::read_to_string(&lock_path)?;
        let Some(toml_after) = with_workspace_version(cargo_toml, &version.to_string()) else {
            return stop("No version in Cargo.toml's [workspace.package].");
        };
        fs::write(self.root.join("Cargo.toml"), toml_after)?;
        if !self.attempt(&cargo(), &["update", "--workspace", "--offline"])? {
            self.restore_version_files()?;
            return stop("cargo could not update Cargo.lock: nothing changed.");
        }
        if fs::read_to_string(&lock_path)? != lock_with_version(&lock_before, &version.to_string())
        {
            self.restore_version_files()?;
            return stop(
                "Cargo.lock changed more than the workspace's own versions: nothing changed.",
            );
        }
        let changed = self.capture("git", &["status", "--porcelain"])?;
        if changed
            .lines()
            .any(|line| !matches!(line.trim_start(), "M Cargo.toml" | "M Cargo.lock"))
        {
            self.restore_version_files()?;
            return stop(format!(
                "Files other than Cargo.toml and Cargo.lock changed:\n{}\nNothing was committed.",
                changed.trim_end()
            ));
        }
        let stat = self.capture("git", &["diff", "--stat"])?;
        self.say(stat.trim_end())
    }

    /// Commits Cargo.toml and Cargo.lock with `message`, asking for another while the hook
    /// refuses it; the end of the input puts the files back.
    fn commit(
        &mut self,
        mut message: String,
        default_message: &str,
        version: &Version,
    ) -> Result<(), Stop> {
        self.step("Committing")?;
        self.run("git", &["add", "--", "Cargo.toml", "Cargo.lock"])?;
        while !self.attempt("git", &["commit", "--message", &message])? {
            self.say(
                "The commit was refused (the hook says why). Another message, or the end of the input \
                 (Ctrl+Z or Ctrl+D) to stop and put the files back:",
            )?;
            let Some(again) = self.read_line()? else {
                self.run(
                    "git",
                    &["reset", "--quiet", "--", "Cargo.toml", "Cargo.lock"],
                )?;
                self.restore_version_files()?;
                return stop("Stopped: nothing changed.");
            };
            message = if again.is_empty() {
                default_message.to_owned()
            } else {
                again
            };
        }
        let commit = self.commit_of("HEAD")?.unwrap_or_default();
        self.done.push(format!(
            "committed {} \"{message}\" (Cargo.toml and Cargo.lock at {version})",
            short(&commit)
        ));
        self.undo.insert(0, "git reset --hard HEAD~1".to_owned());
        Ok(())
    }

    /// Pushes main and `tag` to origin when asked to (`origin_main` is where origin's main was).
    fn push(&mut self, tag: &str, origin_main: Option<&str>) -> Result<(), Stop> {
        self.step("The push")?;
        let push = ["push", "--atomic", "origin", "main", tag];
        let unpushed = match origin_main {
            Some(origin_main) => self.capture(
                "git",
                &["log", "--oneline", &format!("{origin_main}..HEAD")],
            )?,
            None => "(origin has no main yet)".to_owned(),
        };
        let unpushed = if unpushed.trim().is_empty() {
            "(none)"
        } else {
            unpushed.trim_end()
        };
        self.say(&format!(
            "`git {}` sends {tag} and main's commits that origin lacks:\n{unpushed}",
            push.join(" ")
        ))?;
        if !is_yes(
            &self.ask(&format!("Push main and {tag} to origin now? [y/N] "))?,
            false,
        ) {
            return Err(Stop {
                reason: format!("Not pushed. Push them later with `git {}`.", push.join(" ")),
                code: 0,
            });
        }
        if !self.attempt("git", &push)? {
            return stop("The push failed; with --atomic, nothing was pushed.");
        }
        self.say(&format!("\n{tag} is on origin, with main."))
    }

    /// Writes the reason the tool stopped, with what it had done and how to undo it.
    fn report(&mut self, reason: &str) -> std::io::Result<()> {
        writeln!(self.out, "\n{reason}")?;
        if !self.done.is_empty() {
            writeln!(self.out, "\nDone so far:")?;
            for line in &self.done {
                writeln!(self.out, "- {line}")?;
            }
            if !self.undo.is_empty() {
                writeln!(self.out, "To undo it here:")?;
                for line in &self.undo {
                    writeln!(self.out, "  {line}")?;
                }
            }
        }
        Ok(())
    }

    fn say(&mut self, text: &str) -> Result<(), Stop> {
        writeln!(self.out, "{text}")?;
        Ok(())
    }

    fn step(&mut self, title: &str) -> Result<(), Stop> {
        self.say(&format!("\n== {title}"))
    }

    /// The next line of the input, trimmed, or `None` at its end.
    fn read_line(&mut self) -> Result<Option<String>, Stop> {
        self.out.flush()?;
        let mut line = String::new();
        if self.input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        Ok(Some(line.trim().to_owned()))
    }

    /// Asks `question`; the end of the input stops the tool.
    fn ask(&mut self, question: &str) -> Result<String, Stop> {
        write!(self.out, "{question}")?;
        match self.read_line()? {
            Some(answer) => Ok(answer),
            None => stop("Stopped: no answer."),
        }
    }

    fn command(&self, program: &str, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(self.root)
            .stdin(Stdio::null());
        command
    }

    /// Runs a program, showing its output; whether it succeeded.
    fn attempt(&mut self, program: &str, args: &[&str]) -> Result<bool, Stop> {
        self.say(&format!("> {} {}", shown(program), args.join(" ")))?;
        match self.command(program, args).status() {
            Ok(status) => Ok(status.success()),
            Err(error) => stop(format!("{program} could not start: {error}")),
        }
    }

    /// Runs a program and stops when it fails.
    fn run(&mut self, program: &str, args: &[&str]) -> Result<(), Stop> {
        if self.attempt(program, args)? {
            Ok(())
        } else {
            stop(format!("{} {} failed.", shown(program), args.join(" ")))
        }
    }

    /// A program's output, or `None` when it fails; its messages go to the process's.
    fn try_capture(&mut self, program: &str, args: &[&str]) -> Result<Option<String>, Stop> {
        let output = match self
            .command(program, args)
            .stderr(Stdio::inherit())
            .output()
        {
            Ok(output) => output,
            Err(error) => return stop(format!("{program} could not start: {error}")),
        };
        Ok(output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned()))
    }

    /// A program's output; stops when it fails.
    fn capture(&mut self, program: &str, args: &[&str]) -> Result<String, Stop> {
        match self.try_capture(program, args)? {
            Some(output) => Ok(output),
            None => stop(format!("{program} {} failed.", args.join(" "))),
        }
    }

    /// The commit `revision` names in this clone, or `None`.
    fn commit_of(&mut self, revision: &str) -> Result<Option<String>, Stop> {
        let revision = format!("{revision}^{{commit}}");
        Ok(self
            .try_capture("git", &["rev-parse", "--quiet", "--verify", &revision])?
            .map(|commit| commit.trim().to_owned()))
    }

    /// Refs on origin, as `git ls-remote` lists them: (ref, commit).
    fn remote_refs(&mut self, kind: &str) -> Result<Vec<(String, String)>, Stop> {
        let Some(listing) = self.try_capture("git", &["ls-remote", kind, "origin"])? else {
            return stop(
                "origin cannot be reached (git ls-remote failed): check the remote and your network.",
            );
        };
        Ok(listing
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(commit, name)| (name.to_owned(), commit.to_owned()))
            .collect())
    }

    /// Origin's main, or `None` when it has none.
    fn remote_main(&mut self) -> Result<Option<String>, Stop> {
        Ok(self
            .remote_refs("--heads")?
            .into_iter()
            .find(|(name, _)| name == "refs/heads/main")
            .map(|(_, commit)| commit))
    }

    /// The names of origin's tags.
    fn remote_tags(&mut self) -> Result<Vec<String>, Stop> {
        Ok(self
            .remote_refs("--tags")?
            .into_iter()
            .filter_map(|(name, _)| {
                let name = name.strip_prefix("refs/tags/")?;
                // An annotated tag is listed twice, its peeled commit as `name^{}`.
                (!name.ends_with("^{}")).then(|| name.to_owned())
            })
            .collect())
    }

    /// Whether `tag` exists here or on origin.
    fn tag_taken(&mut self, tag: &str, origin_tags: &[String]) -> Result<bool, Stop> {
        Ok(origin_tags.iter().any(|name| name == tag)
            || self.commit_of(&format!("refs/tags/{tag}"))?.is_some())
    }

    /// Whether `commit` is `of` or one of its ancestors, when this clone has it.
    fn is_ancestor(&mut self, commit: &str, of: &str) -> Result<bool, Stop> {
        if commit == of {
            return Ok(true);
        }
        if self.commit_of(commit)?.is_none() {
            return Ok(false);
        }
        Ok(self
            .try_capture("git", &["merge-base", "--is-ancestor", commit, of])?
            .is_some())
    }

    /// Puts Cargo.toml and Cargo.lock back as they were committed.
    fn restore_version_files(&mut self) -> Result<(), Stop> {
        self.run(
            "git",
            &["checkout", "HEAD", "--", "Cargo.toml", "Cargo.lock"],
        )
    }
}

/// Cargo: the one that runs this tool (`cargo run`), or the one on the path.
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// A program as the messages show it: its name, not where Cargo keeps it.
fn shown(program: &str) -> &str {
    Path::new(program)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(program)
}

/// A commit as the messages show it.
fn short(commit: &str) -> &str {
    commit.get(..10).unwrap_or(commit)
}

/// Whether `answer` is yes: y or yes, or nothing when yes is the default.
fn is_yes(answer: &str, by_default: bool) -> bool {
    match answer.trim().to_lowercase().as_str() {
        "" => by_default,
        "y" | "yes" => true,
        _ => false,
    }
}
