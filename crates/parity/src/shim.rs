use crate::{CALLS_FILE, Call, REAL_TOOLS_FILE, executable, hidden, outputs::sha256};
use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// The recording shim, the whole of a recorder's `main`: `kuleshov-record`, or that of a crate built
/// on kuleshov. Copied as `ffmpeg` and `ffprobe` into a folder that the code under test takes for its
/// tools folder, it appends its arguments to `calls.jsonl` next to itself, with the contents of any
/// list file among them, then runs the real tool named in `real-tools.txt` with the same arguments
/// and standard streams, and exits with its code.
///
/// ```no_run
/// fn main() -> std::process::ExitCode {
///     kuleshov_parity::Shim { name: "melies-record", wavs: true }.run()
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Shim {
    /// The recorder's name, which starts its error messages.
    pub name: &'static str,
    /// Also record each existing WAV among the arguments, or named in a concat list, by size and
    /// SHA-256 (`"N bytes, SHA-256 …"`) under the call's `lists`: a WAV that is gone once the call's
    /// output is made can still be compared bit for bit. kuleshov's own recorder does not.
    pub wavs: bool,
}

impl Shim {
    /// Record the call and run the real tool, exiting with its code; 101 when the shim itself fails.
    #[must_use]
    pub fn run(self) -> ExitCode {
        match self.record() {
            Ok(code) => code,
            Err(error) => {
                eprintln!("{}: {error}", self.name);
                ExitCode::from(101)
            }
        }
    }

    fn record(self) -> Result<ExitCode, String> {
        let exe = env::current_exe().map_err(|error| error.to_string())?;
        let dir = exe.parent().ok_or("the shim has no folder")?;
        let tool = exe
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("the shim has no name")?
            .to_owned();
        let real = fs::read_to_string(dir.join(REAL_TOOLS_FILE))
            .map_err(|error| format!("{REAL_TOOLS_FILE}: {error}"))?;
        let real = PathBuf::from(real.trim()).join(executable(&tool));
        let args: Vec<_> = env::args_os().skip(1).collect();

        let call = Call {
            tool,
            args: args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
            lists: self.lists(&args),
        };
        let line = serde_json::to_string(&call).map_err(|error| error.to_string())?;
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(CALLS_FILE))
            .and_then(|mut file| writeln!(file, "{line}"))
            .map_err(|error| format!("{CALLS_FILE}: {error}"))?;

        let status = hidden(&real)
            .args(&args)
            .status()
            .map_err(|error| format!("{}: {error}", real.display()))?;
        Ok(status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .map_or(ExitCode::FAILURE, ExitCode::from))
    }

    /// What a call with `args` records under `lists`: each list file's contents and, with `wavs`,
    /// each WAV's size and SHA-256, by path.
    pub(crate) fn lists(self, args: &[OsString]) -> BTreeMap<String, String> {
        let mut lists = BTreeMap::new();
        for arg in args {
            let path = Path::new(arg);
            let is_list = path
                .extension()
                .is_some_and(|extension| extension == "txt" || extension == "ffconcat");
            if is_list && let Ok(text) = fs::read_to_string(path) {
                if self.wavs {
                    for file in listed_files(&text) {
                        record_wav(&mut lists, &file);
                    }
                }
                lists.insert(arg.to_string_lossy().into_owned(), text);
            } else if self.wavs {
                record_wav(&mut lists, &arg.to_string_lossy());
            }
        }
        lists
    }
}

/// The files a concat list names (`file '…'`, with `'\''` for a quote, as kuleshov writes them).
pub(crate) fn listed_files(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("file '")?.strip_suffix('\''))
        .map(|path| path.replace("'\\''", "'"))
        .collect()
}

/// Record the WAV at `path`, if it is one and exists, by size and SHA-256.
fn record_wav(lists: &mut BTreeMap<String, String>, path: &str) {
    let is_wav = Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"));
    if is_wav && let Ok(bytes) = fs::read(path) {
        lists.insert(
            path.to_owned(),
            format!("{} bytes, SHA-256 {}", bytes.len(), sha256(&bytes)),
        );
    }
}

#[cfg(test)]
#[path = "../tests/unit/shim.rs"]
mod tests;
