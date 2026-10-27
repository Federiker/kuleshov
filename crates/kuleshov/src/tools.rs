//! Where `ffmpeg` and `ffprobe` are: the caller says (explicit paths, folders to search or a
//! resolver), and the bare name, resolved by the system through `PATH`, is the fallback. Kuleshov
//! never bundles, downloads or looks for FFmpeg on its own.

use std::{
    fmt,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

/// One of the two programs kuleshov runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Ffmpeg,
    Ffprobe,
}

impl Tool {
    /// The program's name: `ffmpeg` or `ffprobe`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Ffmpeg => "ffmpeg",
            Self::Ffprobe => "ffprobe",
        }
    }

    /// The program's file name on this platform, such as `ffmpeg.exe` on Windows.
    #[must_use]
    pub fn file_name(self) -> String {
        format!("{}{}", self.name(), std::env::consts::EXE_SUFFIX)
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

type Resolver = dyn Fn(Tool) -> Option<PathBuf> + Send + Sync;

/// Where `ffmpeg` and `ffprobe` are, given to every function that runs them.
///
/// [`Tools::default`] runs the bare names, found on `PATH`.
#[derive(Clone, Default)]
pub struct Tools {
    source: Source,
}

#[derive(Clone, Default)]
enum Source {
    #[default]
    Path,
    Paths {
        ffmpeg: PathBuf,
        ffprobe: PathBuf,
    },
    Resolver(Arc<Resolver>),
}

impl Tools {
    /// The bare names, found on `PATH` when a command starts; the same as [`Tools::default`].
    #[must_use]
    pub fn from_path() -> Self {
        Self::default()
    }

    /// The programs at these paths.
    #[must_use]
    pub fn new(ffmpeg: impl Into<PathBuf>, ffprobe: impl Into<PathBuf>) -> Self {
        Self {
            source: Source::Paths {
                ffmpeg: ffmpeg.into(),
                ffprobe: ffprobe.into(),
            },
        }
    }

    /// Each tool from the first of `dirs` that holds it (`ffmpeg`, `ffmpeg.exe` on Windows), else
    /// its bare name for `PATH`. The folders are searched now, once.
    #[must_use]
    pub fn find_in<I, P>(dirs: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let dirs: Vec<PathBuf> = dirs
            .into_iter()
            .map(|dir| dir.as_ref().to_owned())
            .collect();
        let path = |tool: Tool| find_in(tool, &dirs).unwrap_or_else(|| PathBuf::from(tool.name()));
        Self::new(path(Tool::Ffmpeg), path(Tool::Ffprobe))
    }

    /// Each tool from `resolve`, asked every time a command is built, so that it can follow
    /// settings that change; `None` falls back to the bare name for `PATH`.
    #[must_use]
    pub fn with_resolver(
        resolve: impl Fn(Tool) -> Option<PathBuf> + Send + Sync + 'static,
    ) -> Self {
        Self {
            source: Source::Resolver(Arc::new(resolve)),
        }
    }

    /// The program run for `tool`: a path, or the bare name for `PATH`.
    #[must_use]
    pub fn path(&self, tool: Tool) -> PathBuf {
        match &self.source {
            Source::Path => None,
            Source::Paths { ffmpeg, ffprobe } => Some(match tool {
                Tool::Ffmpeg => ffmpeg.clone(),
                Tool::Ffprobe => ffprobe.clone(),
            }),
            Source::Resolver(resolve) => resolve(tool),
        }
        .unwrap_or_else(|| PathBuf::from(tool.name()))
    }

    /// A command for `tool`. It starts without a console window ([`hide_console`]).
    #[must_use]
    pub fn command(&self, tool: Tool) -> Command {
        let mut command = Command::new(self.path(tool));
        hide_console(&mut command);
        command
    }
}

impl fmt::Debug for Tools {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            Source::Path => f.write_str("Tools(PATH)"),
            Source::Paths { ffmpeg, ffprobe } => f
                .debug_struct("Tools")
                .field("ffmpeg", ffmpeg)
                .field("ffprobe", ffprobe)
                .finish(),
            Source::Resolver(_) => f.write_str("Tools(resolver)"),
        }
    }
}

/// The first existing `<dir>/<tool>[.exe]` of `dirs`, if any.
pub(crate) fn find_in(tool: Tool, dirs: &[PathBuf]) -> Option<PathBuf> {
    let file = tool.file_name();
    dirs.iter()
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file())
}

/// On Windows, start `command` without a console window (`CREATE_NO_WINDOW`).
///
/// A desktop application without a console of its own (`windows_subsystem = "windows"`) would
/// otherwise see every console program it starts open, and flash, a window. Elsewhere, nothing.
pub fn hide_console(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
#[path = "../tests/unit/tools.rs"]
mod tests;
