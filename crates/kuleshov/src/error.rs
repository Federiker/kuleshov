use std::{
    fmt, io,
    path::{Path, PathBuf},
};

/// What went wrong. Each error names the file it is about, where there is one.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The caller asked to stop: the tool was stopped, or never started, and what it had written
    /// was removed.
    Cancelled,
    /// An argument is out of range, such as no input files or odd video dimensions; nothing ran.
    InvalidInput(String),
    /// A path the tools cannot be given, such as a clip path with a line break in a concat list.
    UnsupportedPath { path: PathBuf, reason: &'static str },
    /// The program could not be started, or waited for.
    Run { program: PathBuf, source: io::Error },
    /// The program ran and failed. `code` is `None` when it was stopped by a signal; `stderr` holds
    /// the last lines it wrote ([`crate::stderr_tail`]); `file` is the file it was working on.
    Failed {
        program: PathBuf,
        file: Option<PathBuf>,
        code: Option<i32>,
        stderr: String,
    },
    /// The tool's answer about `path` could not be used, such as no duration or no audio.
    Probe { path: PathBuf, reason: String },
    /// A file or folder could not be created, written or read.
    Io { path: PathBuf, source: io::Error },
}

/// The result of every fallible function here.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Whether the caller cancelled the work.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// The file the error is about, if any.
    #[must_use]
    pub fn file(&self) -> Option<&Path> {
        match self {
            Self::UnsupportedPath { path, .. }
            | Self::Probe { path, .. }
            | Self::Io { path, .. } => Some(path),
            Self::Failed { file, .. } => file.as_deref(),
            Self::Cancelled | Self::InvalidInput(_) | Self::Run { .. } => None,
        }
    }

    /// A failed run as the failure of work on `path`.
    pub(crate) fn on(self, path: &Path) -> Self {
        match self {
            Self::Failed {
                program,
                file: None,
                code,
                stderr,
            } => Self::Failed {
                program,
                file: Some(path.to_owned()),
                code,
                stderr,
            },
            other => other,
        }
    }

    /// An I/O error about `path`.
    pub(crate) fn io(path: &Path) -> impl FnOnce(io::Error) -> Self {
        let path = path.to_owned();
        move |source| Self::Io { path, source }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("cancelled"),
            Self::InvalidInput(message) => f.write_str(message),
            Self::UnsupportedPath { path, reason } => write!(f, "{}: {reason}", path.display()),
            Self::Run { program, source } => {
                write!(f, "could not run {}: {source}", program.display())
            }
            Self::Failed {
                program,
                file,
                code,
                stderr,
            } => {
                let name = program
                    .file_stem()
                    .map_or_else(|| program.to_string_lossy(), |stem| stem.to_string_lossy());
                match file {
                    Some(file) if name == "ffprobe" => {
                        write!(f, "ffprobe could not read {}: ", file.display())?;
                    }
                    Some(file) => write!(f, "{name} failed on {}: ", file.display())?,
                    None => write!(f, "{name} ")?,
                }
                match code {
                    Some(code) => write!(f, "exited with code {code}")?,
                    None => f.write_str("was stopped")?,
                }
                if stderr.is_empty() {
                    Ok(())
                } else {
                    write!(f, ": {stderr}")
                }
            }
            Self::Probe { path, reason } => write!(f, "{}: {reason}", path.display()),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Run { source, .. } | Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/error.rs"]
mod tests;
