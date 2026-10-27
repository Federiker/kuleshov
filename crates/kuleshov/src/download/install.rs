use super::{FfmpegBuild, extract::extract, fetch::fetch};
use crate::{
    Error, Result, Tool, Tools,
    process::{cancelled, stderr_tail},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use uuid::Uuid;

/// Which build the pair in a folder came from, as `scripts/tools.ts` writes it too: absent when
/// the pair was put there by hand.
pub(crate) const RECORD: &str = "download.json";

impl Tools {
    /// `ffmpeg` and `ffprobe` from `dir`, installing there first the build pinned for this
    /// platform ([`FfmpegBuild::pinned`]) when they are missing.
    ///
    /// # Errors
    /// [`Error::InvalidInput`] where no build is pinned (use [`Tools::find_or_download_build`]),
    /// and the errors of [`Tools::find_or_download_build`].
    pub fn find_or_download(
        dir: &Path,
        progress: impl FnMut(u64, Option<u64>),
        cancel: Option<&AtomicBool>,
    ) -> Result<Self> {
        let build = FfmpegBuild::pinned().ok_or_else(|| {
            Error::InvalidInput(format!(
                "no FFmpeg build is pinned for {}-{}: name one with find_or_download_build",
                std::env::consts::OS,
                std::env::consts::ARCH
            ))
        })?;
        Self::find_or_download_build(dir, &build, progress, cancel)
    }

    /// `ffmpeg` and `ffprobe` from `dir`, downloading `build` there first when they are missing.
    ///
    /// A pair already in `dir` is kept: one put there by hand always, one installed from a build
    /// (recorded in `download.json`, as `mise run tools` records it) while that build is still
    /// `build`; otherwise `build` is downloaded, its SHA-256 checked, and only `ffmpeg` and
    /// `ffprobe` are taken out of it. `progress` gets the bytes received and the total, when the
    /// server gives it. The pair in place is replaced only once the new one is complete, so a
    /// failed or cancelled download leaves it, and nothing else, behind. Both programs must then
    /// run (`-version`).
    ///
    /// Nothing else in kuleshov downloads: call this where the program may use the network.
    ///
    /// # Errors
    /// [`Error::Download`] when the server, the checksum or the archive fail,
    /// [`Error::InvalidInput`] for a malformed checksum, [`Error::Io`], [`Error::Cancelled`], and
    /// [`Error::Run`] or [`Error::Failed`] when a program does not run.
    pub fn find_or_download_build(
        dir: &Path,
        build: &FfmpegBuild,
        mut progress: impl FnMut(u64, Option<u64>),
        cancel: Option<&AtomicBool>,
    ) -> Result<Self> {
        let paths = [Tool::Ffmpeg, Tool::Ffprobe].map(|tool| dir.join(tool.file_name()));
        let record = fs::read_to_string(dir.join(RECORD))
            .ok()
            .and_then(|text| serde_json::from_str::<FfmpegBuild>(&text).ok());
        let present = paths.iter().all(|path| path.is_file());
        if !present || record.is_some_and(|record| record != *build) {
            install(dir, build, &paths, &mut progress, cancel)?;
        }
        for path in &paths {
            runs(path)?;
        }
        let [ffmpeg, ffprobe] = paths;
        Ok(Self::new(ffmpeg, ffprobe))
    }
}

fn install(
    dir: &Path,
    build: &FfmpegBuild,
    paths: &[PathBuf; 2],
    progress: &mut dyn FnMut(u64, Option<u64>),
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    if cancelled(cancel) {
        return Err(Error::Cancelled);
    }
    fs::create_dir_all(dir).map_err(Error::io(dir))?;
    let id = Uuid::new_v4();
    let archive = dir.join(format!(".kuleshov-{id}.download"));
    let parts = [Tool::Ffmpeg, Tool::Ffprobe]
        .map(|tool| dir.join(format!(".kuleshov-{id}-{}", tool.file_name())));
    let result = (|| {
        fetch(build, &archive, progress, cancel)?;
        let members = [
            (Tool::Ffmpeg.file_name(), parts[0].as_path()),
            (Tool::Ffprobe.file_name(), parts[1].as_path()),
        ];
        extract(build, &archive, &members, cancel)?;
        for part in &parts {
            executable(part)?;
        }
        // The record goes first, so that a pair half replaced is never taken for `build`'s.
        let record = dir.join(RECORD);
        match fs::remove_file(&record) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                return Err(Error::io(&record)(error));
            }
            _ => {}
        }
        for (part, path) in parts.iter().zip(paths) {
            fs::rename(part, path).map_err(Error::io(path))?;
        }
        let text = serde_json::to_string_pretty(build).unwrap_or_default() + "\n";
        fs::write(&record, text).map_err(Error::io(&record))
    })();
    for leftover in std::iter::once(&archive).chain(&parts) {
        let _ = fs::remove_file(leftover);
    }
    result
}

/// Mark `path` executable (Unix); elsewhere nothing to do.
#[cfg_attr(
    not(unix),
    expect(clippy::unnecessary_wraps, reason = "it can fail on Unix")
)]
fn executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(Error::io(path))?;
    }
    let _ = path;
    Ok(())
}

/// Whether the program at `path` runs: `-version` succeeds.
fn runs(path: &Path) -> Result<()> {
    let mut command = std::process::Command::new(path);
    crate::hide_console(&mut command);
    let output = command
        .arg("-version")
        .output()
        .map_err(|source| Error::Run {
            program: path.to_owned(),
            source,
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::Failed {
            program: path.to_owned(),
            file: Some(path.to_owned()),
            code: output.status.code(),
            stderr: stderr_tail(&output.stderr),
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/download/install.rs"]
mod tests;
