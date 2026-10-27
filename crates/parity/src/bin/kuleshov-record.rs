//! The recording shim. Copied as `ffmpeg` and `ffprobe` into a folder that the code under test
//! takes for its tools folder, it appends its arguments to `calls.jsonl` next to itself, with the
//! contents of any list file among them, then runs the real tool named in `real-tools.txt` with the
//! same arguments and standard streams, and exits with its code.

use kuleshov_parity::{CALLS_FILE, Call, REAL_TOOLS_FILE, executable, hidden};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("kuleshov-record: {error}");
            ExitCode::from(101)
        }
    }
}

fn run() -> Result<ExitCode, String> {
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

    let mut lists = BTreeMap::new();
    for arg in &args {
        let path = Path::new(arg);
        let is_list = path
            .extension()
            .is_some_and(|extension| extension == "txt" || extension == "ffconcat");
        if is_list && let Ok(text) = fs::read_to_string(path) {
            lists.insert(arg.to_string_lossy().into_owned(), text);
        }
    }
    let call = Call {
        tool,
        args: args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        lists,
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
