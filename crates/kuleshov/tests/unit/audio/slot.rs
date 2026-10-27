#![expect(
    clippy::cast_precision_loss,
    reason = "sample positions of a two-second slot"
)]

use super::*;
use crate::audio_duration;
use crate::support::tools;

fn tone(dir: &Path, secs: f64) -> PathBuf {
    let path = dir.join(format!("{}.wav", Uuid::new_v4()));
    let status = tools()
        .command(crate::Tool::Ffmpeg)
        .args(["-y", "-f", "lavfi", "-i"])
        .arg(format!("sine=frequency=440:duration={secs}"))
        .arg(&path)
        .output()
        .unwrap()
        .status;
    assert!(status.success());
    path
}

#[test]
fn slot_has_the_exact_length_with_lead_and_trail_silence() {
    let dir = std::env::temp_dir().join(format!("kuleshov-slot-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let slot = place_in_slot(&tools(), &tone(&dir, 1.0), 0.5, 2.0, 24_000, &dir).unwrap();
    assert!((audio_duration(&tools(), &slot).unwrap() - 2.0).abs() < 0.01);
    // The tone sits from 0.5 s to 1.5 s, with silence before and after it.
    let samples = wav_samples(&slot);
    let sound = |sample: &i16| sample.unsigned_abs() > 500;
    let first = samples.iter().position(sound).unwrap() as f64 / 24_000.0;
    let last = samples.iter().rposition(sound).unwrap() as f64 / 24_000.0;
    assert!((first - 0.5).abs() < 0.01, "the sound starts at {first} s");
    assert!((last - 1.5).abs() < 0.01, "the sound ends at {last} s");
    fs::remove_dir_all(dir).unwrap();
}

/// The 16-bit samples of a mono WAV file (FFmpeg may put other chunks before `data`).
fn wav_samples(path: &Path) -> Vec<i16> {
    let bytes = fs::read(path).unwrap();
    let at = bytes
        .windows(4)
        .position(|window| window == b"data")
        .unwrap()
        + 8;
    bytes[at..]
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

#[test]
fn overlong_audio_is_cut_to_the_slot() {
    let dir = std::env::temp_dir().join(format!("kuleshov-slot-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let slot = place_in_slot(&tools(), &tone(&dir, 3.0), 0.0, 1.0, 24_000, &dir).unwrap();
    assert!((audio_duration(&tools(), &slot).unwrap() - 1.0).abs() < 0.01);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn concatenated_slots_add_up() {
    let dir = std::env::temp_dir().join(format!("kuleshov-slot-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let a = place_in_slot(&tools(), &tone(&dir, 0.5), 0.2, 1.0, 24_000, &dir).unwrap();
    let b = place_in_slot(&tools(), &tone(&dir, 0.5), 0.2, 1.5, 24_000, &dir).unwrap();
    let joined = concat_wav(&tools(), &[a, b], 24_000, &dir).unwrap();
    assert!((audio_duration(&tools(), &joined).unwrap() - 2.5).abs() < 0.01);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejects_empty_input_and_zero_slots() {
    let dir = std::env::temp_dir().join(format!("kuleshov-slot-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let error = concat_wav(&tools(), &[], 24_000, &dir).unwrap_err();
    assert!(error.to_string().contains("no input files"), "{error:#}");
    // A real input, so only the slot's length is wrong.
    let error = place_in_slot(&tools(), &tone(&dir, 0.5), 0.0, 0.0, 24_000, &dir).unwrap_err();
    assert!(
        error.to_string().contains("slot length must be positive"),
        "{error:#}"
    );
    fs::remove_dir_all(dir).unwrap();
}
