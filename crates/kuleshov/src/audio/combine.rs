use crate::{Error, Result, Tool, Tools, concat::concat_entry, process::run_cancellable};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use uuid::Uuid;

/// Combine multiple audio files into a single mono MP3.
///
/// Resamples the output to `rate` Hz and writes it to a generated file under
/// `data_dir/audio`.
///
/// # Errors
/// As [`combine_audio_files_with_options`].
pub fn combine_audio_files(
    tools: &Tools,
    files: &[PathBuf],
    rate: u32,
    data_dir: &Path,
) -> Result<PathBuf> {
    combine_audio_files_with_cancel(tools, files, rate, data_dir, None)
}

/// Cancellable version of [`combine_audio_files`].
///
/// # Errors
/// As [`combine_audio_files_with_options`].
pub fn combine_audio_files_with_cancel(
    tools: &Tools,
    files: &[PathBuf],
    rate: u32,
    data_dir: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    combine_audio_files_with_options(
        tools,
        files,
        rate,
        data_dir,
        &CombineOptions::default(),
        cancel,
    )
}

/// How the combined MP3 is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp3Encoding {
    /// Variable bitrate at LAME quality `0` (best) to `9`; quality 2 is about 100-130 kbit/s for speech.
    Quality(u8),
    /// Constant bitrate in kbit/s, as audiobook retailers ask for (for example 192).
    Cbr(u32),
}

/// Quiet background noise, the "room tone" retailers want instead of digital silence.
///
/// FFmpeg seeds the noise at random, so two encodes with room tone differ in their samples
/// (docs/parity.md).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomTone {
    /// Room tone placed before the first sound, in seconds.
    pub lead_secs: f64,
    /// Level of the noise in dBFS RMS; retailers require the noise floor to stay below -60 dBFS.
    pub level_dbfs: f32,
}

/// The list file FFmpeg's concat demuxer reads: one `file '<path>'` line per input, each path as
/// given ([`concat_entry`]).
pub(crate) fn concat_list(files: &[PathBuf]) -> Result<String> {
    Ok(files
        .iter()
        .map(|path| concat_entry(path, path))
        .collect::<Result<Vec<_>>>()?
        .join("\n"))
}

/// Encoding choices for [`combine_audio_files_with_options`].
#[derive(Debug, Clone, PartialEq)]
pub struct CombineOptions {
    pub encoding: Mp3Encoding,
    /// Add a lead-in and a constant noise floor; `None` leaves the audio as it is.
    pub room_tone: Option<RoomTone>,
    /// ID3 tags written to the MP3 as `(name, value)`, for example `("comment", ...)`.
    pub tags: Vec<(String, String)>,
}

impl Default for CombineOptions {
    fn default() -> Self {
        Self {
            encoding: Mp3Encoding::Quality(2),
            room_tone: None,
            tags: Vec::new(),
        }
    }
}

/// Peak amplitude of FFmpeg's pink noise for an RMS of `level_dbfs`; the exact offset is
/// confirmed by the measurement test in `tests/unit/audio/analyze.rs`.
fn noise_amplitude(level_dbfs: f32) -> f32 {
    10.0f32.powf((level_dbfs + NOISE_PEAK_OFFSET_DB) / 20.0)
}

/// How far a pink-noise RMS sits below its `amplitude` setting.
const NOISE_PEAK_OFFSET_DB: f32 = 16.2;

/// Combine files into one mono MP3 with explicit encoding options.
///
/// The MP3 is written to a generated name under `data_dir/audio`, with a temporary concat list
/// beside it that is always removed; a cancelled or failed encode removes its partial output.
///
/// # Errors
/// [`Error::InvalidInput`] without files, [`Error::UnsupportedPath`] for a path the list cannot hold
/// (not valid Unicode, or with a line break), [`Error::Io`] when the folder or the list cannot be
/// written, [`Error::Cancelled`], and the errors of [`run_cancellable`].
pub fn combine_audio_files_with_options(
    tools: &Tools,
    files: &[PathBuf],
    rate: u32,
    data_dir: &Path,
    options: &CombineOptions,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    if files.is_empty() {
        return Err(Error::InvalidInput("no input files provided".to_owned()));
    }
    let list = concat_list(files)?;

    let out_dir = data_dir.join("audio");
    fs::create_dir_all(&out_dir).map_err(Error::io(&out_dir))?;

    let output_path = out_dir.join(format!("{}.mp3", Uuid::new_v4()));

    // Create concat list file
    let list_path = out_dir.join(format!("{}.txt", Uuid::new_v4()));
    if let Err(error) = fs::write(&list_path, list) {
        let _ = fs::remove_file(&list_path);
        return Err(Error::io(&list_path)(error));
    }

    // ffmpeg command
    let mut command = tools.command(Tool::Ffmpeg);
    command
        .args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path);
    if let Some(tone) = options.room_tone.as_ref() {
        // The noise runs forever; `amix=duration=first` ends it together with the speech.
        command
            .args(["-f", "lavfi", "-i"])
            .arg(format!(
                "anoisesrc=color=pink:amplitude={}:sample_rate={rate}",
                noise_amplitude(tone.level_dbfs)
            ))
            .arg("-filter_complex")
            .arg(format!(
                "[0:a]aresample={rate},aformat=channel_layouts=mono,adelay={}:all=1[voice];\
[voice][1:a]amix=inputs=2:duration=first:normalize=0:dropout_transition=0[mix]",
                (tone.lead_secs * 1000.0).round()
            ))
            .args(["-map", "[mix]"]);
    }
    command
        .args(["-ac", "1", "-ar"])
        .arg(rate.to_string())
        .args(["-acodec", "mp3"]);
    match options.encoding {
        Mp3Encoding::Quality(quality) => {
            command.arg("-q:a").arg(quality.min(9).to_string());
        }
        Mp3Encoding::Cbr(kbps) => {
            command.arg("-b:a").arg(format!("{kbps}k"));
        }
    }
    for (name, value) in &options.tags {
        command.arg("-metadata").arg(format!("{name}={value}"));
    }
    command.arg(&output_path);
    let result = run_cancellable(&mut command, cancel);
    let _ = fs::remove_file(&list_path);
    if let Err(error) = result {
        let _ = fs::remove_file(&output_path);
        return Err(error.on(&output_path));
    }

    Ok(output_path)
}

/// Async wrapper for [`combine_audio_files`], running its blocking process work
/// on Tokio's blocking thread pool.
///
/// # Errors
/// As [`combine_audio_files`]; [`Error::Cancelled`] when the runtime drops the task.
#[cfg(feature = "tokio")]
pub async fn combine_audio_files_async(
    tools: &Tools,
    files: Vec<PathBuf>,
    rate: u32,
    data_dir: PathBuf,
) -> Result<PathBuf> {
    let tools = tools.clone();
    join(
        tokio::task::spawn_blocking(move || combine_audio_files(&tools, &files, rate, &data_dir))
            .await,
    )
}

/// The result of a blocking task: its panic goes on in the caller, as it would have without the
/// pool; a task dropped by a runtime shutting down is cancelled.
#[cfg(feature = "tokio")]
pub(crate) fn join<T>(joined: std::result::Result<Result<T>, tokio::task::JoinError>) -> Result<T> {
    match joined {
        Ok(result) => result,
        Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
        Err(_) => Err(Error::Cancelled),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/audio/combine.rs"]
mod tests;
