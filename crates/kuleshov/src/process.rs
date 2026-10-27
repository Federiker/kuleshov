use crate::{Error, Result};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

/// How many of its last standard-error lines a failed tool's error message keeps.
const STDERR_TAIL_LINES: usize = 8;

/// Whether `cancel` asks to stop.
pub(crate) fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
}

/// Run a child process, polling `cancel` and terminating the child when requested.
///
/// The child's standard error is collected (it would otherwise go to a console a desktop app does
/// not have): an unsuccessful exit becomes [`Error::Failed`] with its last lines. A command that
/// is already cancelled is never started.
///
/// # Errors
/// [`Error::Cancelled`], [`Error::Run`] when it cannot be started or waited for, and
/// [`Error::Failed`] when it exits unsuccessfully.
pub fn run_cancellable(command: &mut Command, cancel: Option<&AtomicBool>) -> Result<ExitStatus> {
    if cancelled(cancel) {
        return Err(Error::Cancelled);
    }
    let program = PathBuf::from(command.get_program());
    let mut child = match command.stderr(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(source) => return Err(Error::Run { program, source }),
    };
    // Drained on its own thread so a chatty tool never blocks on a full pipe.
    let stderr = child.stderr.take().map(|mut pipe| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            bytes
        })
    });
    let status = loop {
        if cancelled(cancel) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Run { program, source });
            }
        }
    };
    let bytes = stderr
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    if status.success() {
        Ok(status)
    } else {
        Err(Error::Failed {
            program,
            file: None,
            code: status.code(),
            stderr: stderr_tail(&bytes),
        })
    }
}

/// Run `command` to its end and return its standard output; a failure is about `file`.
pub(crate) fn output(command: &mut Command, file: &Path) -> Result<Vec<u8>> {
    let program = PathBuf::from(command.get_program());
    let output = match command.output() {
        Ok(output) => output,
        Err(source) => return Err(Error::Run { program, source }),
    };
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(Error::Failed {
            program,
            file: Some(file.to_owned()),
            code: output.status.code(),
            stderr: stderr_tail(&output.stderr),
        })
    }
}

/// The last few non-empty lines a tool wrote to standard error, joined by " / ".
///
/// `ffmpeg` rewrites its progress line with carriage returns, so those end a line too.
#[must_use]
pub fn stderr_tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    lines[lines.len().saturating_sub(STDERR_TAIL_LINES)..].join(" / ")
}

#[cfg(test)]
#[path = "../tests/unit/process.rs"]
mod tests;
