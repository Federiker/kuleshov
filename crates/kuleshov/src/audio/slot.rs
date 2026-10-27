use crate::{Error, Result, Tool, Tools, audio::combine::concat_list, process::run_cancellable};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use uuid::Uuid;

/// Place `input` inside a silent WAV of exactly `slot_secs`, starting after
/// `lead_secs` of silence. Audio that does not fit is cut at the slot end.
///
/// WAV output keeps lengths sample-exact, which MP3 encoders do not, so slots can be
/// chained to stay in sync with video frames. Written to `out_dir`.
///
/// # Errors
/// As [`place_in_slot_with_cancel`].
pub fn place_in_slot(
    tools: &Tools,
    input: &Path,
    lead_secs: f64,
    slot_secs: f64,
    rate: u32,
    out_dir: &Path,
) -> Result<PathBuf> {
    place_in_slot_with_cancel(tools, input, lead_secs, slot_secs, rate, out_dir, None)
}

/// Cancellable version of [`place_in_slot`].
///
/// # Errors
/// [`Error::InvalidInput`] for a slot that is not positive, [`Error::Io`] when `out_dir` cannot be
/// created, [`Error::Cancelled`], and the errors of [`run_cancellable`] (naming `input`).
pub fn place_in_slot_with_cancel(
    tools: &Tools,
    input: &Path,
    lead_secs: f64,
    slot_secs: f64,
    rate: u32,
    out_dir: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    if slot_secs.is_nan() || slot_secs <= 0.0 {
        return Err(Error::InvalidInput(
            "slot length must be positive".to_owned(),
        ));
    }
    fs::create_dir_all(out_dir).map_err(Error::io(out_dir))?;
    let output = out_dir.join(format!("{}.wav", Uuid::new_v4()));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a lead of milliseconds, never negative"
    )]
    let delay_ms = (lead_secs.max(0.0) * 1000.0).round() as u64;
    let filter = format!(
        "aresample={rate},aformat=channel_layouts=mono,adelay={delay_ms},apad=whole_dur={slot_secs}"
    );
    let result = run_ffmpeg(
        tools,
        &[
            "-y".into(),
            "-i".into(),
            input.into(),
            "-af".into(),
            filter.into(),
            "-t".into(),
            slot_secs.to_string().into(),
            "-c:a".into(),
            "pcm_s16le".into(),
            output.as_os_str().into(),
        ],
        cancel,
    );
    if let Err(error) = result {
        let _ = fs::remove_file(&output);
        return Err(error.on(input));
    }
    Ok(output)
}

/// Join WAV files (all at `rate`, mono) end to end into one WAV in `out_dir`.
///
/// # Errors
/// As [`concat_wav_with_cancel`].
pub fn concat_wav(tools: &Tools, files: &[PathBuf], rate: u32, out_dir: &Path) -> Result<PathBuf> {
    concat_wav_with_cancel(tools, files, rate, out_dir, None)
}

/// Cancellable version of [`concat_wav`]. Its temporary concat list is always removed, and a
/// partial output when the join fails or is cancelled.
///
/// # Errors
/// [`Error::InvalidInput`] without files, [`Error::UnsupportedPath`] for a path the list cannot hold,
/// [`Error::Io`] when the folder or the list cannot be
/// written, [`Error::Cancelled`], and the errors of [`run_cancellable`].
pub fn concat_wav_with_cancel(
    tools: &Tools,
    files: &[PathBuf],
    rate: u32,
    out_dir: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<PathBuf> {
    if files.is_empty() {
        return Err(Error::InvalidInput("no input files provided".to_owned()));
    }
    let contents = concat_list(files)?;
    fs::create_dir_all(out_dir).map_err(Error::io(out_dir))?;
    let list = out_dir.join(format!("{}.txt", Uuid::new_v4()));
    if let Err(error) = fs::write(&list, contents) {
        let _ = fs::remove_file(&list);
        return Err(Error::io(&list)(error));
    }
    let output = out_dir.join(format!("{}.wav", Uuid::new_v4()));
    let result = run_ffmpeg(
        tools,
        &[
            "-y".into(),
            "-f".into(),
            "concat".into(),
            "-safe".into(),
            "0".into(),
            "-i".into(),
            list.as_os_str().into(),
            "-ar".into(),
            rate.to_string().into(),
            "-ac".into(),
            "1".into(),
            "-c:a".into(),
            "pcm_s16le".into(),
            output.as_os_str().into(),
        ],
        cancel,
    );
    let _ = fs::remove_file(&list);
    if let Err(error) = result {
        let _ = fs::remove_file(&output);
        return Err(error.on(&output));
    }
    Ok(output)
}

fn run_ffmpeg(tools: &Tools, args: &[OsString], cancel: Option<&AtomicBool>) -> Result<()> {
    let mut command = tools.command(Tool::Ffmpeg);
    command.args(args);
    run_cancellable(&mut command, cancel)?;
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/audio/slot.rs"]
mod tests;
