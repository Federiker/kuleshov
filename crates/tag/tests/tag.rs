//! `mise run tag` on a repository of its own: a workspace of one crate, cloned from a bare origin
//! in the system's temporary folder, with the answers given as input.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// A workspace at 0.1.0 with one commit, pushed to its origin; removed when dropped.
struct Repo {
    root: PathBuf,
    work: PathBuf,
    origin: PathBuf,
}

impl Drop for Repo {
    fn drop(&mut self) {
        // Best effort: a leftover folder in the temporary one does no harm.
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper; a failure should fail the test"
)]
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper; a failure should fail the test"
)]
fn repo(name: &str) -> Repo {
    let root = std::env::temp_dir().join(format!("kuleshov-tag-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let work = root.join("work");
    let origin = root.join("origin.git");
    fs::create_dir_all(work.join("crates/a/src")).unwrap();
    fs::create_dir_all(&origin).unwrap();
    git(
        &origin,
        &["init", "--quiet", "--bare", "--initial-branch=main"],
    );
    git(&work, &["init", "--quiet", "--initial-branch=main"]);
    for (key, value) in [
        ("user.name", "Tester"),
        ("user.email", "tester@example.com"),
        ("commit.gpgsign", "false"),
        ("tag.gpgsign", "false"),
        ("core.autocrlf", "false"),
        ("core.hooksPath", ".git/hooks"),
    ] {
        git(&work, &["config", key, value]);
    }
    fs::write(
        work.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        work.join("crates/a/Cargo.toml"),
        "[package]\nname = \"a\"\nversion.workspace = true\nedition.workspace = true\n",
    )
    .unwrap();
    fs::write(work.join("crates/a/src/lib.rs"), "").unwrap();
    fs::write(work.join(".gitignore"), "target/\n").unwrap();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let lock = Command::new(cargo)
        .args(["generate-lockfile", "--offline"])
        .current_dir(&work)
        .status()
        .unwrap();
    assert!(lock.success());
    git(&work, &["add", "."]);
    git(&work, &["commit", "--quiet", "--message", "[init] Init"]);
    git(
        &work,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&work, &["push", "--quiet", "origin", "main"]);
    Repo { root, work, origin }
}

/// Runs the tool in `repo` with `answers`: (exit code, what it wrote).
#[expect(
    clippy::unwrap_used,
    reason = "a test helper; a failure should fail the test"
)]
fn tag(repo: &Repo, answers: &str) -> (u8, String) {
    let mut out = Vec::new();
    let code = kuleshov_tag::run(&repo.work, &mut answers.as_bytes(), &mut out);
    (code, String::from_utf8(out).unwrap())
}

fn origin_tags(repo: &Repo) -> String {
    git(&repo.origin, &["tag", "--list"])
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper; a failure should fail the test"
)]
fn version_in_cargo_toml(repo: &Repo) -> String {
    let text = fs::read_to_string(repo.work.join("Cargo.toml")).unwrap();
    kuleshov_tag::workspace_version(&text).unwrap().to_owned()
}

#[test]
fn the_first_tag_is_the_current_version_on_head() {
    let repo = repo("first");
    let head = git(&repo.work, &["rev-parse", "HEAD"]);
    let (code, out) = tag(&repo, "\n\ny\n");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("never tagged (no v0.1.0)"), "{out}");
    assert!(out.contains("no commit, the tag goes on HEAD"), "{out}");
    assert_eq!(origin_tags(&repo).trim(), "v0.1.0");
    assert_eq!(git(&repo.work, &["rev-parse", "v0.1.0^{commit}"]), head);
    assert_eq!(
        git(&repo.work, &["cat-file", "-t", "v0.1.0"]).trim(),
        "tag",
        "annotated"
    );
}

#[test]
fn a_new_version_is_committed_tagged_and_pushed() {
    let repo = repo("bump");
    assert_eq!(tag(&repo, "\n\ny\n").0, 0);
    let (code, out) = tag(&repo, "\n\n\ny\n");
    assert_eq!(code, 0, "{out}");
    assert_eq!(version_in_cargo_toml(&repo), "0.1.1");
    let lock = fs::read_to_string(repo.work.join("Cargo.lock")).unwrap();
    assert!(lock.contains("name = \"a\"\nversion = \"0.1.1\""), "{lock}");
    assert_eq!(
        git(&repo.work, &["log", "-1", "--format=%s"]).trim(),
        "[release] kuleshov 0.1.1"
    );
    assert_eq!(
        git(
            &repo.work,
            &["tag", "--list", "--format=%(contents:subject)", "v0.1.1"]
        )
        .trim(),
        "kuleshov 0.1.1"
    );
    assert_eq!(origin_tags(&repo), "v0.1.0\nv0.1.1\n");
    assert_eq!(
        git(&repo.origin, &["rev-parse", "main"]),
        git(&repo.work, &["rev-parse", "HEAD"])
    );
    assert!(git(&repo.work, &["status", "--porcelain"]).is_empty());
}

#[test]
fn wrong_versions_are_asked_again_and_the_push_can_wait() {
    let repo = repo("wrong");
    git(&repo.work, &["tag", "v0.1.0"]);
    git(&repo.work, &["tag", "v0.2.0"]);
    let (code, out) = tag(
        &repo,
        "abc\n0.0.9\n0.1.0\n0.2.0\n0.3.0\n[release] Version 0.3.0\n\nn\n",
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("abc is not a version"), "{out}");
    assert!(out.contains("0.0.9 is not after 0.1.0."), "{out}");
    assert!(out.contains("0.1.0 is not after 0.1.0."), "{out}");
    assert!(out.contains("v0.2.0 exists already"), "{out}");
    assert!(
        out.contains("Not pushed. Push them later with `git push --atomic origin main v0.3.0`.")
    );
    assert!(
        out.contains("To undo it here:\n  git tag -d v0.3.0\n  git reset --hard HEAD~1"),
        "{out}"
    );
    assert_eq!(version_in_cargo_toml(&repo), "0.3.0");
    assert_eq!(
        git(&repo.work, &["log", "-1", "--format=%s"]).trim(),
        "[release] Version 0.3.0"
    );
    assert!(origin_tags(&repo).is_empty());
}

#[test]
fn it_stops_before_changing_anything_unless_ready() {
    let repo = repo("refusals");
    let (code, out) = tag(&repo, "");
    assert_eq!(code, 1);
    assert!(out.contains("Stopped: no answer."), "{out}");

    let (code, out) = tag(&repo, "0.2.0\n\nn\n");
    assert_eq!(code, 1);
    assert!(out.contains("Stopped: nothing changed."), "{out}");
    assert_eq!(version_in_cargo_toml(&repo), "0.1.0");

    fs::write(repo.work.join("notes.txt"), "draft").unwrap();
    let (code, out) = tag(&repo, "\n\ny\n");
    assert_eq!(code, 1);
    assert!(
        out.contains("Uncommitted changes or untracked files"),
        "{out}"
    );
    fs::remove_file(repo.work.join("notes.txt")).unwrap();

    git(&repo.work, &["switch", "--quiet", "--create", "side"]);
    let (code, out) = tag(&repo, "\n\ny\n");
    assert_eq!(code, 1);
    assert!(out.contains("HEAD is on side"), "{out}");
    git(&repo.work, &["switch", "--quiet", "--detach", "main"]);
    let (code, out) = tag(&repo, "\n\ny\n");
    assert_eq!(code, 1);
    assert!(out.contains("HEAD is detached"), "{out}");
    git(&repo.work, &["switch", "--quiet", "main"]);

    git(
        &repo.work,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "--message",
            "[docs] Later",
        ],
    );
    git(&repo.work, &["push", "--quiet", "origin", "main"]);
    git(&repo.work, &["reset", "--quiet", "--hard", "HEAD~1"]);
    let (code, out) = tag(&repo, "\n\ny\n");
    assert_eq!(code, 1);
    assert!(
        out.contains("has commits this main lacks: pull them first"),
        "{out}"
    );
    assert!(origin_tags(&repo).is_empty());
}

#[test]
fn a_refused_commit_asks_for_another_message() {
    let repo = repo("hook");
    let hook = repo.work.join(".git/hooks/commit-msg");
    fs::write(&hook, "#!/bin/sh\ngrep -q '^\\[release\\] ' \"$1\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (code, out) = tag(&repo, "0.2.0\nNo prefix\n\nAlso none\n\nn\n");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.matches("The commit was refused").count(), 2, "{out}");
    assert_eq!(
        git(&repo.work, &["log", "-1", "--format=%s"]).trim(),
        "[release] kuleshov 0.2.0"
    );

    let (code, out) = tag(&repo, "0.3.0\nNo prefix\n\n");
    assert_eq!(code, 1);
    assert!(out.contains("Stopped: nothing changed."), "{out}");
    assert_eq!(version_in_cargo_toml(&repo), "0.2.0");
    assert!(git(&repo.work, &["status", "--porcelain"]).is_empty());
}
