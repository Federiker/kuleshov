use crate::{Error, Result, Tool, Tools, process::run_cancellable};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use uuid::Uuid;

/// Mux an audio track with a video's first video stream into a unique MP4.
///
/// `output_path` is a directory. The result ends when the shorter input ends;
/// `fps`, when supplied, sets the output video frame rate.
///
/// # Errors
/// As [`add_audio_to_video_with_cancel`].
pub fn add_audio_to_video(
    tools: &Tools,
    video_path: &Path,
    audio_path: &Path,
    fps: Option<u32>,
    output_path: &Path,
) -> Result<PathBuf> {
    add_audio_to_video_with_cancel(tools, video_path, audio_path, fps, output_path, None)
}

/// Mux audio while copying an MP4-compatible video stream without changing its frame rate.
/// Callers must supply an already normalized video; audio is encoded once as AAC.
/// Supply `duration` for known frame-aligned tracks to avoid `-shortest` dropping
/// a final copied packet while the audio encoder drains. Otherwise stop at the shorter input.
///
/// # Errors
/// [`Error::InvalidInput`] for a duration that is not finite and positive, [`Error::Io`] when the
/// folder cannot be created, [`Error::Cancelled`], and the errors of [`run_cancellable`] (naming
/// the video). A failed or cancelled mux removes its partial output.
pub fn mux_audio_with_cancel(
    tools: &Tools,
    video_path: &Path,
    audio_path: &Path,
    duration: Option<f64>,
    output_path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    mux_audio(
        tools,
        video_path,
        audio_path,
        None,
        duration,
        output_path,
        cancel,
        true,
    )
}

/// Cancellable version of [`add_audio_to_video`].
///
/// # Errors
/// As [`mux_audio_with_cancel`].
pub fn add_audio_to_video_with_cancel(
    tools: &Tools,
    video_path: &Path,
    audio_path: &Path,
    fps: Option<u32>,
    output_path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    mux_audio(
        tools,
        video_path,
        audio_path,
        fps,
        None,
        output_path,
        cancel,
        false,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the two public variants' arguments"
)]
fn mux_audio(
    tools: &Tools,
    video_path: &Path,
    audio_path: &Path,
    fps: Option<u32>,
    duration: Option<f64>,
    output_path: &Path,
    cancel: Option<&AtomicBool>,
    copy_video: bool,
) -> Result<PathBuf> {
    if let Some(duration) = duration
        && !(duration.is_finite() && duration > 0.0)
    {
        return Err(Error::InvalidInput(
            "mux duration must be finite and positive".to_owned(),
        ));
    }
    std::fs::create_dir_all(output_path).map_err(Error::io(output_path))?;
    let output_path = output_path.join(format!("{}.mp4", Uuid::new_v4()));

    let mut cmd = tools.command(Tool::Ffmpeg);

    cmd.args(["-y", "-i"]);
    cmd.arg(video_path)
        .arg("-i")
        .arg(audio_path)
        .args(["-map", "0:v:0", "-map", "1:a:0", "-c:a", "aac"]);
    if let Some(duration) = duration {
        cmd.arg("-t").arg(duration.to_string());
    } else {
        cmd.arg("-shortest");
    }
    if copy_video {
        cmd.args(["-c:v", "copy"]);
    } else {
        cmd.args(["-c:v", "libx264", "-preset", "medium"]);
    }

    if let Some(fps) = fps {
        cmd.args(["-r", &fps.to_string()]);
    }

    cmd.arg(&output_path);

    if let Err(error) = run_cancellable(&mut cmd, cancel) {
        let _ = std::fs::remove_file(&output_path);
        return Err(error.on(video_path));
    }

    Ok(output_path)
}
