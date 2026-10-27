use crate::{Call, Outcome, Reference, Scenario, Value, describe};
use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// File, next to the recording shim, that names the folder holding the real tools.
pub const REAL_TOOLS_FILE: &str = "real-tools.txt";
/// File, next to the recording shim, where it appends one JSON [`Call`] per line.
pub const CALLS_FILE: &str = "calls.jsonl";

/// `name` with the platform's executable suffix.
#[must_use]
pub fn executable(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// A command for `program` that opens no console window on Windows.
#[must_use]
pub fn hidden(program: &Path) -> Command {
    let mut command = Command::new(program);
    hide_console(&mut command);
    command
}

/// On Windows, start `command` without a console window (`CREATE_NO_WINDOW`); elsewhere, nothing.
fn hide_console(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Runs scenarios one after the other through the recording shim and collects what each did.
pub struct Session {
    shim: PathBuf,
    tools: PathBuf,
    work: PathBuf,
    /// Paths returned by earlier scenarios and their names, such as `{combine#0}`.
    outputs: Vec<(String, String)>,
    scenarios: Vec<Scenario>,
}

impl Session {
    /// Prepare `work` (emptied first): a `shim` folder where `recorder` (the `kuleshov-record`
    /// executable) is copied as `ffmpeg` and `ffprobe`, running the real ones in `tools`.
    ///
    /// # Errors
    /// When the folders or the copies cannot be made.
    pub fn new(recorder: &Path, tools: &Path, work: &Path) -> io::Result<Self> {
        if work.exists() {
            fs::remove_dir_all(work)?;
        }
        let shim = work.join("shim");
        fs::create_dir_all(&shim)?;
        for tool in ["ffmpeg", "ffprobe"] {
            fs::copy(recorder, shim.join(executable(tool)))?;
        }
        let tools = std::path::absolute(tools)?;
        fs::write(
            shim.join(REAL_TOOLS_FILE),
            tools.to_string_lossy().as_bytes(),
        )?;
        Ok(Self {
            shim,
            tools,
            work: work.to_path_buf(),
            outputs: Vec::new(),
            scenarios: Vec::new(),
        })
    }

    /// The folder to give the code under test as its tools folder.
    #[must_use]
    pub fn shim(&self) -> &Path {
        &self.shim
    }

    /// The folder holding the real tools.
    #[must_use]
    pub fn tools(&self) -> &Path {
        &self.tools
    }

    /// The scenario folder for `name` (each scenario writes there).
    #[must_use]
    pub fn dir(&self, name: &str) -> PathBuf {
        self.work.join(name)
    }

    /// The `index`th file returned by the earlier scenario `name`.
    ///
    /// # Panics
    /// When that scenario returned no such file: the scenarios are out of order.
    #[must_use]
    pub fn output(&self, name: &str, index: usize) -> PathBuf {
        let wanted = format!("{{{name}#{index}}}");
        let found = self.outputs.iter().find(|(_, symbol)| *symbol == wanted);
        #[expect(clippy::expect_used, reason = "scenarios name only earlier outputs")]
        PathBuf::from(&found.expect("an earlier scenario's output").0)
    }

    /// Run the scenario `name`: `body` gets its folder (created) and calls one public function.
    ///
    /// # Errors
    /// When the shim's log or an output cannot be read.
    pub fn run(&mut self, name: &str, body: impl FnOnce(&Path) -> Outcome) -> io::Result<()> {
        let dir = self.dir(name);
        fs::create_dir_all(&dir)?;
        let _ = fs::remove_file(self.shim.join(CALLS_FILE));
        let outcome = body(&dir);
        let log = fs::read_to_string(self.shim.join(CALLS_FILE)).unwrap_or_default();
        let _ = fs::remove_file(self.shim.join(CALLS_FILE));
        let mut uuids = Vec::new();
        let returned: Vec<(String, String)> = outcome
            .outputs
            .iter()
            .enumerate()
            .map(|(index, path)| {
                (
                    path.to_string_lossy().into_owned(),
                    format!("{{{name}#{index}}}"),
                )
            })
            .collect();
        self.outputs.extend(returned.iter().cloned());
        let mut calls = Vec::new();
        for line in log.lines().filter(|line| !line.trim().is_empty()) {
            let call: Call = serde_json::from_str(line).map_err(io::Error::other)?;
            calls.push(Call {
                tool: call.tool,
                args: call
                    .args
                    .iter()
                    .map(|arg| self.normalise(arg, &mut uuids))
                    .collect(),
                lists: call
                    .lists
                    .iter()
                    .map(|(arg, text)| {
                        (
                            self.normalise(arg, &mut uuids),
                            self.normalise(text, &mut uuids),
                        )
                    })
                    .collect(),
            });
        }
        let mut outputs = Vec::new();
        for (path, symbol) in &returned {
            outputs.push(describe(&self.tools, Path::new(path), symbol.clone())?);
        }
        let mut files = Vec::new();
        list_files(&dir, &mut files)?;
        let mut files: Vec<String> = files
            .iter()
            .map(|path| self.normalise(&path.to_string_lossy(), &mut uuids))
            .collect();
        files.sort();
        let result = match outcome.value {
            Some(Ok(text)) => Value::Ok(self.normalise(&text, &mut uuids)),
            Some(Err(text)) => Value::Err(self.normalise(&text, &mut uuids)),
            None => Value::Ok(String::new()),
        };
        self.scenarios.push(Scenario {
            name: name.to_owned(),
            calls,
            result,
            outputs,
            files,
        });
        Ok(())
    }

    /// Everything recorded, under `source`.
    ///
    /// # Errors
    /// When the real tools cannot report their versions.
    pub fn finish(self, source: &str) -> io::Result<Reference> {
        let version = |tool: &str| -> io::Result<String> {
            let output = hidden(&self.tools.join(executable(tool)))
                .arg("-version")
                .output()?;
            Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned())
        };
        Ok(Reference {
            source: source.to_owned(),
            ffmpeg: version("ffmpeg")?,
            ffprobe: version("ffprobe")?,
            scenarios: self.scenarios,
        })
    }

    /// `text` with machine-specific parts named (docs/parity.md): returned files as `{scenario#n}`,
    /// the work folder as `{work}`, generated UUIDs as `{uuid1}`, `{uuid2}`… in order of appearance.
    fn normalise(&self, text: &str, uuids: &mut Vec<String>) -> String {
        let mut text = text.to_owned();
        // Longest first, so that no path is cut by a shorter one it starts with.
        let mut outputs: Vec<&(String, String)> = self.outputs.iter().collect();
        outputs.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
        for (path, symbol) in outputs {
            text = text.replace(path.as_str(), symbol);
        }
        let work = self.work.to_string_lossy();
        text = text.replace(work.as_ref(), "{work}");
        name_addresses(&name_uuids(&text, uuids))
    }
}

/// `text` with every UUID replaced by `{uuidN}`, numbered across a scenario by first appearance.
pub(crate) fn name_uuids(text: &str, uuids: &mut Vec<String>) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 36 <= bytes.len() && is_uuid(&bytes[i..i + 36]) {
            let uuid = &text[i..i + 36];
            let number = if let Some(position) = uuids.iter().position(|seen| seen == uuid) {
                position + 1
            } else {
                uuids.push(uuid.to_owned());
                uuids.len()
            };
            let _ = write!(out, "{{uuid{number}}}");
            i += 36;
        } else {
            let character = text[i..].chars().next().unwrap_or_default();
            out.push(character);
            i += character.len_utf8().max(1);
        }
    }
    out
}

/// `text` with the memory addresses FFmpeg prints in its log prefixes (`[in#0 @ 000001a61a1e2540]`,
/// `[mp3 @ 0x55d0c8a4b2c0]`) written as `{address}`: they change on every run.
pub(crate) fn name_addresses(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(" @ ") {
        out.push_str(&rest[..at + 3]);
        rest = &rest[at + 3..];
        let end = rest.find(']').unwrap_or(0);
        let address = rest[..end].strip_prefix("0x").unwrap_or(&rest[..end]);
        if !address.is_empty() && address.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            out.push_str("{address}");
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    out
}

fn is_uuid(bytes: &[u8]) -> bool {
    bytes.iter().enumerate().all(|(i, byte)| match i {
        8 | 13 | 18 | 23 => *byte == b'-',
        _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
    })
}

fn list_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            list_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/session.rs"]
mod tests;
