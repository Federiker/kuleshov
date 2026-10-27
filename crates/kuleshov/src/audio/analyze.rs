use crate::{
    Error, Result, Tool, Tools,
    process::{self, cancelled},
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::AtomicBool,
};

/// Measurements of an audio file, as decoded from the file itself.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioAnalysis {
    pub codec: String,
    /// Bit rate reported for the audio stream, in bit/s.
    pub bit_rate: Option<u32>,
    pub sample_rate: u32,
    pub channels: u32,
    pub duration_secs: f64,
    /// RMS of the whole file in dBFS, pauses included.
    pub rms_dbfs: f32,
    /// Highest sample of the decoded audio in dBFS.
    pub peak_dbfs: f32,
    /// RMS of the quietest 100 ms of the file in dBFS.
    pub noise_floor_dbfs: f32,
    /// Time before the first sound louder than -45 dBFS, in seconds.
    pub lead_secs: f64,
    /// Time after the last such sound, in seconds.
    pub tail_secs: f64,
}

/// Decoded rate used for the measurements.
const RATE: usize = 44_100;
/// Short block used to find where sound starts and ends: 20 ms.
const BLOCK: usize = RATE / 50;
/// Window used for the noise floor: 100 ms.
const WINDOW: usize = RATE / 10;
/// Sound counts from -45 dBFS RMS in a 20 ms block.
const SOUND_POWER: f64 = 3.162_277_7e-5;
/// Anything quieter is reported as this, so digital silence does not become minus infinity.
const FLOOR_DBFS: f32 = -120.0;

#[expect(clippy::cast_possible_truncation, reason = "decibels fit an f32")]
fn dbfs(power: f64) -> f32 {
    if power <= 1.0e-12 {
        FLOOR_DBFS
    } else {
        (10.0 * power.log10()) as f32
    }
}

/// Running measurements over a stream of mono samples.
#[derive(Default)]
struct Meter {
    total_power: f64,
    count: u64,
    peak: f32,
    block_power: f64,
    block_count: usize,
    block_index: u64,
    first_sound: Option<u64>,
    last_sound: Option<u64>,
    window_power: f64,
    window_count: usize,
    quietest_window: Option<f64>,
}

impl Meter {
    #[expect(
        clippy::cast_precision_loss,
        reason = "block and window sizes are small"
    )]
    fn push(&mut self, sample: f32) {
        let power = f64::from(sample) * f64::from(sample);
        self.total_power += power;
        self.count += 1;
        self.peak = self.peak.max(sample.abs());
        self.block_power += power;
        self.block_count += 1;
        if self.block_count == BLOCK {
            if self.block_power / BLOCK as f64 > SOUND_POWER {
                self.first_sound.get_or_insert(self.block_index);
                self.last_sound = Some(self.block_index);
            }
            self.block_index += 1;
            self.block_power = 0.0;
            self.block_count = 0;
        }
        self.window_power += power;
        self.window_count += 1;
        if self.window_count == WINDOW {
            let mean = self.window_power / WINDOW as f64;
            self.quietest_window = Some(self.quietest_window.map_or(mean, |q| q.min(mean)));
            self.window_power = 0.0;
            self.window_count = 0;
        }
    }
}

/// Measure the level, noise floor and lead-in/tail of an audio file with FFmpeg.
///
/// The file is decoded as it is read, so even a long chapter needs little memory.
///
/// # Errors
/// When ffprobe or ffmpeg cannot run or read the file, or it holds no audio; the error names the
/// file.
pub fn analyze_audio(tools: &Tools, path: &Path) -> Result<AudioAnalysis> {
    analyze_audio_with_cancel(tools, path, None)
}

/// Cancellable version of [`analyze_audio`]: a long file is decoded for seconds, and `cancel` stops
/// the decoder between two reads.
///
/// # Errors
/// As [`analyze_audio`], and [`Error::Cancelled`].
#[expect(
    clippy::cast_precision_loss,
    reason = "sample counts of an audio file are far below 2^52"
)]
#[expect(clippy::too_many_lines, reason = "manuzio's function, kept whole")]
pub fn analyze_audio_with_cancel(
    tools: &Tools,
    path: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<AudioAnalysis> {
    if cancelled(cancel) {
        return Err(Error::Cancelled);
    }
    let probe = process::output(
        tools
            .command(Tool::Ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=codec_name,bit_rate,sample_rate,channels",
                "-of",
                "default=noprint_wrappers=1",
            ])
            .arg(path),
        path,
    )?;
    let text = String::from_utf8_lossy(&probe);
    let value = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .map(str::trim)
    };

    if cancelled(cancel) {
        return Err(Error::Cancelled);
    }
    let mut command = tools.command(Tool::Ffmpeg);
    let program = PathBuf::from(command.get_program());
    let mut child = command
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "f32le", "-ac", "1", "-ar", &RATE.to_string(), "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| Error::Run {
            program: program.clone(),
            source,
        })?;
    let decode_failed = |code| Error::Failed {
        program: program.clone(),
        file: Some(path.to_owned()),
        code,
        stderr: String::new(),
    };
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(decode_failed(None));
    };
    let mut meter = Meter::default();
    let mut buffer = vec![0u8; 1 << 16];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        if cancelled(cancel) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Cancelled);
        }
        let read = match stdout.read(&mut buffer) {
            Ok(read) => read,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::io(path)(error));
            }
        };
        if read == 0 {
            break;
        }
        carry.extend_from_slice(&buffer[..read]);
        let whole = carry.len() / 4 * 4;
        for bytes in carry[..whole].chunks_exact(4) {
            meter.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        }
        carry.drain(..whole);
    }
    let status = child.wait().map_err(|source| Error::Run {
        program: program.clone(),
        source,
    })?;
    if !status.success() {
        return Err(decode_failed(status.code()));
    }
    if meter.count == 0 {
        return Err(Error::Probe {
            path: path.to_owned(),
            reason: "holds no audio".to_owned(),
        });
    }

    let duration_secs = meter.count as f64 / RATE as f64;
    let seconds = |blocks: u64| blocks as f64 * BLOCK as f64 / RATE as f64;
    let (lead_secs, tail_secs) = match (meter.first_sound, meter.last_sound) {
        (Some(first), Some(last)) => (seconds(first), duration_secs - seconds(last + 1)),
        _ => (duration_secs, duration_secs),
    };
    let mean_power = meter.total_power / meter.count as f64;
    Ok(AudioAnalysis {
        codec: value("codec_name").unwrap_or_default().to_owned(),
        bit_rate: value("bit_rate").and_then(|v| v.parse().ok()),
        sample_rate: value("sample_rate")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        channels: value("channels").and_then(|v| v.parse().ok()).unwrap_or(0),
        duration_secs,
        rms_dbfs: dbfs(mean_power),
        peak_dbfs: dbfs(f64::from(meter.peak) * f64::from(meter.peak)),
        noise_floor_dbfs: dbfs(meter.quietest_window.unwrap_or(mean_power)),
        lead_secs,
        tail_secs: tail_secs.max(0.0),
    })
}

#[cfg(test)]
#[path = "../../tests/unit/audio/analyze.rs"]
mod tests;
