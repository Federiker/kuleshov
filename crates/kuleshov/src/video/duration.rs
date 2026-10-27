use crate::{Error, Result, Tool, Tools, process};
use std::path::Path;

/// Return a video's probed duration in seconds.
///
/// # Errors
/// When ffprobe cannot run or read the file, or gives no duration; the error names the file.
pub fn video_duration(tools: &Tools, path: &Path) -> Result<f64> {
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

    String::from_utf8_lossy(&stdout)
        .trim()
        .parse()
        .map_err(|_| Error::Probe {
            path: path.to_owned(),
            reason: "ffprobe gave no duration".to_owned(),
        })
}
