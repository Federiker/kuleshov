use crate::{Output, hidden};
use sha2::{Digest, Sha256};
use std::{fmt::Write as _, fs, io, path::Path};

/// Lowercase hexadecimal SHA-256 of `bytes`.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// Standard output of a tool in `tools`, or an error with its standard error.
fn run(tools: &Path, tool: &str, args: &[&str], file: &Path) -> io::Result<String> {
    let output = hidden(&tools.join(crate::executable(tool)))
        .args(args.iter().take_while(|arg| **arg != "{}"))
        .arg(file)
        .args(args.iter().skip_while(|arg| **arg != "{}").skip(1))
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{tool} could not read {}: {}",
            file.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Describe the file at `path`, named `name`, with the real tools in `tools`: its bytes, its decoded
/// audio (`-f md5`) and video (`-f framemd5`), and ffprobe's stream parameters.
///
/// # Errors
/// When the file cannot be read or a tool fails on it.
pub fn describe(tools: &Path, path: &Path, name: String) -> io::Result<Output> {
    let bytes = fs::read(path)?;
    let probe = run(
        tools,
        "ffprobe",
        &[
            "-v",
            "error",
            "-show_entries",
            "stream=index,codec_type,codec_name,profile,codec_tag_string,width,height,pix_fmt,\
level,sample_aspect_ratio,r_frame_rate,avg_frame_rate,time_base,start_pts,duration_ts,nb_frames,\
sample_fmt,sample_rate,channels,channel_layout,bit_rate,extradata_size:format=format_name,\
duration,bit_rate:format_tags:stream_tags",
            "-of",
            "compact",
            "{}",
        ],
        path,
    )?;
    let streams: Vec<String> = probe
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    let has = |kind: &str| {
        streams
            .iter()
            .any(|line| line.contains(&format!("|codec_type={kind}")))
    };
    let audio_md5 = if has("audio") {
        let text = run(
            tools,
            "ffmpeg",
            &["-v", "error", "-i", "{}", "-map", "0:a:0", "-f", "md5", "-"],
            path,
        )?;
        Some(text.trim().to_owned())
    } else {
        None
    };
    let (video_framemd5, frames) = if has("video") {
        let text = run(
            tools,
            "ffmpeg",
            &[
                "-v", "error", "-i", "{}", "-map", "0:v:0", "-f", "framemd5", "-",
            ],
            path,
        )?;
        // Line endings as written, so the hash is the same however Git checks the tools out.
        let text = text.replace("\r\n", "\n");
        let frames = text
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .count();
        (Some(sha256(text.as_bytes())), u64::try_from(frames).ok())
    } else {
        (None, None)
    };
    Ok(Output {
        name,
        bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        sha256: sha256(&bytes),
        audio_md5,
        video_framemd5,
        frames,
        streams,
    })
}
