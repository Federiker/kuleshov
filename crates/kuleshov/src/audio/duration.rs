use crate::{Error, Result, Tool, Tools, process};
use std::path::Path;

/// Return the media duration in seconds, as reported by `ffprobe`.
///
/// # Errors
/// When ffprobe cannot run or read the file, or gives no duration; the error names the file.
pub fn audio_duration(tools: &Tools, path: &Path) -> Result<f64> {
    let stdout = process::output(
        tools
            .command(Tool::Ffprobe)
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            // The path as the system gives it: a name that is not valid Unicode is no panic.
            .arg(path),
        path,
    )?;

    std::str::from_utf8(&stdout)
        .ok()
        .and_then(|text| text.trim().parse::<f64>().ok())
        .ok_or_else(|| Error::Probe {
            path: path.to_owned(),
            reason: "ffprobe gave no duration".to_owned(),
        })
}

/// Async wrapper for [`audio_duration`], running its blocking process work on
/// Tokio's blocking thread pool.
///
/// # Errors
/// As [`audio_duration`]; [`Error::Cancelled`] when the runtime drops the task.
#[cfg(feature = "tokio")]
pub async fn audio_duration_async(
    tools: &Tools,
    path: impl Into<std::path::PathBuf>,
) -> Result<f64> {
    let (tools, path) = (tools.clone(), path.into());
    crate::audio::combine::join(
        tokio::task::spawn_blocking(move || audio_duration(&tools, &path)).await,
    )
}

#[cfg(test)]
#[path = "../../tests/unit/audio/duration.rs"]
mod tests;
