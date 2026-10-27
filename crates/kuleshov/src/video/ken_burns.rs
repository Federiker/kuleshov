use crate::{Error, Result, Tool, Tools, process::run_cancellable};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Camera movement applied when turning a still image into a video clip.
pub enum Direction {
    ZoomIn,
    ZoomOut,
    LeftToRight,
    RightToLeft,
}

fn invalid(message: &str) -> Error {
    Error::InvalidInput(message.to_owned())
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the frame count is checked to fit a u32 first"
)]
fn motion_filter(
    width: u32,
    height: u32,
    duration: f64,
    direction: Direction,
    zoom_factor: Option<f64>,
    fps: u32,
) -> Result<(String, u32)> {
    if !(width > 0 && height > 0 && width.is_multiple_of(2) && height.is_multiple_of(2)) {
        return Err(invalid("video dimensions must be positive and even"));
    }
    if fps == 0 {
        return Err(invalid("frame rate must be positive"));
    }
    if !duration.is_finite() {
        return Err(invalid("duration must be finite"));
    }
    let frame_count = (duration.max(0.1) * f64::from(fps)).round().max(1.0);
    if frame_count > f64::from(u32::MAX) {
        return Err(invalid("duration is too long"));
    }
    let frames = frame_count as u32;
    let zoom = zoom_factor.unwrap_or(match direction {
        Direction::ZoomIn | Direction::ZoomOut => 1.25,
        _ => 1.10,
    });
    if !(zoom.is_finite() && zoom <= 10.0) {
        return Err(invalid("zoom factor must be finite and at most 10"));
    }
    let zoom = zoom.max(1.01);
    // Integer crop geometry is less visible on a supersampled working image.
    // Bound its area so larger caller-supplied output sizes do not multiply memory unchecked.
    let pixels = u64::from(width) * u64::from(height);
    let factor = (1..=4_u32)
        .rev()
        .find(|factor| pixels <= 36_000_000 / u64::from(factor * factor))
        .ok_or_else(|| invalid("video dimensions exceed the working image limit"))?;
    let w = width * factor;
    let h = height * factor;
    let denominator = frames.saturating_sub(1).max(1);
    let position = format!("on/{denominator}");
    let (z, x) = match direction {
        Direction::ZoomIn => (
            format!("1+({zoom}-1)*{position}"),
            "(iw-iw/zoom)/2".to_string(),
        ),
        Direction::ZoomOut => (
            format!("{zoom}-({zoom}-1)*{position}"),
            "(iw-iw/zoom)/2".to_string(),
        ),
        Direction::LeftToRight => (zoom.to_string(), format!("(iw-iw/zoom)*{position}")),
        Direction::RightToLeft => (zoom.to_string(), format!("(iw-iw/zoom)*(1-{position})")),
    };
    Ok((
        format!(
            "scale={width}:{height}:force_original_aspect_ratio=increase,\
crop={width}:{height},setsar=1,scale={w}:{h}:flags=bicubic,format=yuv444p,\
zoompan=z='{z}':x='{x}':y='(ih-ih/zoom)/2':d={frames}:s={width}x{height}:fps={fps}"
        ),
        frames,
    ))
}

/// Render a still image as a unique MP4 with zoom or pan motion.
///
/// `output_path` is a directory. Zoom motion defaults to a 1.25 factor and pan
/// to 1.10; duration is clamped to at least 0.1 seconds. The generated clip has
/// no audio.
///
/// # Errors
/// As [`make_motion_from_still_with_cancel`].
#[expect(
    clippy::too_many_arguments,
    reason = "manuzio's signature, plus the tools"
)]
pub fn make_motion_from_still(
    tools: &Tools,
    input_file: &Path,
    width: u32,
    height: u32,
    duration: f64,
    direction: Direction,
    zoom_factor: Option<f64>,
    fps: u32,
    output_path: &Path,
) -> Result<PathBuf> {
    make_motion_from_still_with_cancel(
        tools,
        input_file,
        width,
        height,
        duration,
        direction,
        zoom_factor,
        fps,
        output_path,
        None,
    )
}

/// Cancellable version of [`make_motion_from_still`].
///
/// # Errors
/// [`Error::InvalidInput`] for odd or zero dimensions, a zero frame rate, a duration or zoom that
/// is not finite, or a size beyond the working image limit; [`Error::Io`] when the folder cannot be
/// created; [`Error::Cancelled`]; and the errors of [`run_cancellable`] (naming the image). A
/// failed or cancelled encode removes its partial output.
#[expect(
    clippy::too_many_arguments,
    reason = "manuzio's signature, plus the tools"
)]
pub fn make_motion_from_still_with_cancel(
    tools: &Tools,
    input_file: &Path,
    width: u32,
    height: u32,
    duration: f64,
    direction: Direction,
    zoom_factor: Option<f64>,
    fps: u32,
    output_path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    let (filter, frames) = motion_filter(width, height, duration, direction, zoom_factor, fps)?;
    encode_motion(
        tools,
        input_file,
        &filter,
        frames,
        fps,
        &["-b:v", "8M"],
        output_path,
        cancel,
    )
}

/// Fades of a clip that is joined to others by stream copy, so its encode is the final one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipFades {
    /// Seconds faded in from black at the start; 0 for none.
    pub fade_in: f64,
    /// Seconds faded out to black at the end; 0 for none.
    pub fade_out: f64,
}

/// Like [`make_motion_from_still_with_cancel`] with `fades` applied, encoded once at x264's
/// quality 23 (CRF), as the final video: clips made this way with the same size and frame rate
/// can be joined by [`crate::concat_matching_video_files_with_cancel`] without another encode.
///
/// # Errors
/// As [`make_motion_from_still_with_cancel`].
#[expect(
    clippy::too_many_arguments,
    reason = "manuzio's signature, plus the tools"
)]
pub fn make_faded_motion_from_still_with_cancel(
    tools: &Tools,
    input_file: &Path,
    width: u32,
    height: u32,
    duration: f64,
    direction: Direction,
    fades: ClipFades,
    fps: u32,
    output_path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    let (mut filter, frames) = motion_filter(width, height, duration, direction, None, fps)?;
    filter += &fade_filter(fades, frames, fps);
    encode_motion(
        tools,
        input_file,
        &filter,
        frames,
        fps,
        &["-crf", "23"],
        output_path,
        cancel,
    )
}

/// The fades of a clip of `frames` frames, appended to its motion filter.
pub(crate) fn fade_filter(fades: ClipFades, frames: u32, fps: u32) -> String {
    let mut filter = String::new();
    if fades.fade_in > 0.0 {
        let _ = write!(filter, ",fade=t=in:st=0:d={}", fades.fade_in);
    }
    if fades.fade_out > 0.0 {
        let length = f64::from(frames) / f64::from(fps);
        let _ = write!(
            filter,
            ",fade=t=out:st={}:d={}",
            (length - fades.fade_out).max(0.0),
            fades.fade_out
        );
    }
    filter
}

#[expect(clippy::too_many_arguments, reason = "the motion and its encode")]
fn encode_motion(
    tools: &Tools,
    input_file: &Path,
    filter: &str,
    frames: u32,
    fps: u32,
    rate: &[&str],
    output_path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(output_path).map_err(Error::io(output_path))?;
    let out_path = output_path.join(format!("{}.mp4", Uuid::new_v4()));
    let mut cmd = tools.command(Tool::Ffmpeg);
    cmd.args(["-y", "-loop", "1", "-framerate"])
        .arg(fps.to_string())
        .arg("-i")
        .arg(input_file)
        .arg("-vf")
        .arg(filter)
        .arg("-frames:v")
        .arg(frames.to_string());
    cmd.args(["-c:v", "libx264", "-preset", "medium"])
        .args(rate)
        .args(["-pix_fmt", "yuv420p", "-an"]);
    cmd.arg(&out_path);

    if let Err(error) = run_cancellable(&mut cmd, cancel) {
        let _ = std::fs::remove_file(&out_path);
        return Err(error.on(input_file));
    }

    Ok(out_path)
}

#[cfg(test)]
#[path = "../../tests/unit/video/ken_burns.rs"]
mod tests;
