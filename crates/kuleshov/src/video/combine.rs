use crate::{
    Error, Result, Tool, Tools,
    process::{self, cancelled, run_cancellable},
    video_duration,
};
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use uuid::Uuid;

/// Copy matching H.264 clips without fades or re-encoding.
///
/// Returns `None` when normalization is needed; probe/process failures remain errors.
/// Checks dimensions, frame rate, codec parameters, time base and codec extradata.
/// Inputs must be zero-based constant-frame-rate clips, such as motion helper outputs.
///
/// # Errors
/// [`Error::InvalidInput`] without clips or with a size or frame rate of zero, an ffprobe failure
/// naming the clip, [`Error::UnsupportedPath`] for a clip path that is not valid Unicode or holds a
/// line break (the concat list cannot hold it), [`Error::Io`], [`Error::Cancelled`], and the errors
/// of [`run_cancellable`]. The temporary concat list is always removed, and a partial output when
/// the copy fails or is cancelled.
pub fn concat_matching_video_files_with_cancel(
    tools: &Tools,
    input_files: &[PathBuf],
    width: u32,
    height: u32,
    fps: u32,
    output_path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<Option<PathBuf>> {
    if input_files.is_empty() {
        return Err(Error::InvalidInput("No input files provided".to_owned()));
    }
    if width == 0 || height == 0 || fps == 0 {
        return Err(Error::InvalidInput(
            "video dimensions and frame rate must be positive".to_owned(),
        ));
    }
    let mut signature = None;
    for file in input_files {
        if cancelled(cancel) {
            return Err(Error::Cancelled);
        }
        let probe = process::output(
            tools
                .command(Tool::Ffprobe)
                .args([
                    "-v", "error", "-select_streams", "v:0",
                    "-show_entries",
                    "stream=codec_name,profile,codec_tag_string,width,height,pix_fmt,level,sample_aspect_ratio,r_frame_rate,avg_frame_rate,time_base,start_time,field_order,color_range,color_space,color_transfer,color_primaries,chroma_location,extradata_hash",
                    "-show_data_hash", "sha256", "-of", "default=noprint_wrappers=1",
                ])
                .arg(file),
            file,
        )?;
        let current = String::from_utf8(probe).map_err(|_| Error::Probe {
            path: file.clone(),
            reason: "ffprobe gave clip metadata that is not UTF-8".to_owned(),
        })?;
        let required = [
            "codec_name=h264".to_string(),
            format!("width={width}"),
            format!("height={height}"),
            format!("r_frame_rate={fps}/1"),
            format!("avg_frame_rate={fps}/1"),
            "start_time=0.000000".to_string(),
        ];
        if !required
            .iter()
            .all(|entry| current.lines().any(|line| line == entry))
            || !current
                .lines()
                .any(|line| line.starts_with("extradata_hash=SHA256:"))
            || signature
                .as_ref()
                .is_some_and(|previous| previous != &current)
        {
            return Ok(None);
        }
        signature = Some(current);
    }
    std::fs::create_dir_all(output_path).map_err(Error::io(output_path))?;
    let list = output_path.join(format!("{}.ffconcat", Uuid::new_v4()));
    let output = output_path.join(format!("{}.mp4", Uuid::new_v4()));
    let contents = input_files
        .iter()
        .map(|file| {
            let absolute = std::path::absolute(file).map_err(Error::io(file))?;
            let name = absolute.to_str().ok_or_else(|| Error::UnsupportedPath {
                path: file.clone(),
                reason: "a clip path that is not valid Unicode cannot go in a concat list",
            })?;
            if name.contains(['\n', '\r']) {
                return Err(Error::UnsupportedPath {
                    path: file.clone(),
                    reason: "a clip path with a line break cannot go in a concat list",
                });
            }
            Ok(format!("file '{}'", name.replace('\'', "'\\''")))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    if let Err(error) = std::fs::write(&list, contents) {
        let _ = std::fs::remove_file(&list);
        return Err(Error::io(&list)(error));
    }
    let mut command = tools.command(Tool::Ffmpeg);
    command
        .args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list)
        .args(["-map", "0:v:0", "-c:v", "copy", "-an"])
        .arg(&output);
    let result = run_cancellable(&mut command, cancel);
    let _ = std::fs::remove_file(&list);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&output);
        return Err(error.on(&output));
    }
    Ok(Some(output))
}

/// Normalize and concatenate video clips into a unique silent MP4.
///
/// `output_path` is a directory. Clips are scaled and padded to `width` by
/// `height`, encoded at `fps`, and optionally faded between clips using
/// `fade_in_ms`.
///
/// # Errors
/// As [`combine_video_files_with_cancel`].
pub fn combine_video_files(
    tools: &Tools,
    input_files: &[PathBuf],
    width: u32,
    height: u32,
    fade_in_ms: u32,
    fps: u32,
    output_path: &Path,
) -> Result<PathBuf> {
    combine_video_files_with_cancel(
        tools,
        input_files,
        width,
        height,
        fade_in_ms,
        fps,
        output_path,
        None,
    )
}

/// Cancellable version of [`combine_video_files`].
///
/// # Errors
/// [`Error::InvalidInput`] without clips, [`Error::Io`] when the folder cannot be created, an
/// ffprobe failure naming the clip whose duration a fade needs, [`Error::Cancelled`], and the errors
/// of [`run_cancellable`]. A failed or cancelled encode removes its partial output.
#[expect(
    clippy::too_many_arguments,
    reason = "manuzio's signature, plus the tools"
)]
pub fn combine_video_files_with_cancel(
    tools: &Tools,
    input_files: &[PathBuf],
    width: u32,
    height: u32,
    fade_in_ms: u32,
    fps: u32,
    output_path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(output_path).map_err(Error::io(output_path))?;
    let output_path = output_path.join(format!("{}.mp4", Uuid::new_v4()));

    if input_files.is_empty() {
        return Err(Error::InvalidInput("No input files provided".to_owned()));
    }

    let fade_seconds = f64::from(fade_in_ms) / 1000.0;

    // --- Build filter_complex with scale, pad, and optional fades ---
    let mut filter_complex = String::new();
    for (i, file) in input_files.iter().enumerate() {
        let mut filters = format!(
            "[{i}:v]scale=w={width}:h={height}:force_original_aspect_ratio=decrease,\
pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1"
        );

        if fade_seconds > 0.0 {
            // Fade in for every clip except first
            if i > 0 {
                let _ = write!(filters, ",fade=t=in:st=0:d={fade_seconds}");
            }
            // Fade out for every clip except last
            if i < input_files.len() - 1 {
                let _ = write!(
                    filters,
                    ",fade=t=out:st={start}:d={fade}",
                    start = (video_duration(tools, file)? - fade_seconds).max(0.0),
                    fade = fade_seconds
                );
            }
        }

        let _ = write!(filters, " [v{i}];");
        filter_complex.push_str(&filters);
    }

    // Concatenate all video streams
    for i in 0..input_files.len() {
        let _ = write!(filter_complex, "[v{i}]");
    }
    let _ = write!(filter_complex, "concat=n={}:v=1:a=0 [v]", input_files.len());

    // --- Build FFmpeg command ---
    let mut cmd = tools.command(Tool::Ffmpeg);

    for file in input_files {
        cmd.arg("-i").arg(file);
    }

    cmd.args([
        "-y",
        "-filter_complex",
        &filter_complex,
        "-map",
        "[v]",
        "-r",
        &fps.to_string(),
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-an",
    ])
    .arg(&output_path);

    if let Err(error) = run_cancellable(&mut cmd, cancel) {
        let _ = std::fs::remove_file(&output_path);
        return Err(error.on(&output_path));
    }

    Ok(output_path)
}
