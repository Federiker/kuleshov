#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    reason = "test signals and sample counts are small; digital silence is exactly the floor"
)]

use super::*;
use crate::{
    CombineOptions, Mp3Encoding, RoomTone, combine_audio_files_with_options, support::tools,
};
use std::{fs, path::PathBuf};

/// A 24 kHz mono 16-bit WAV: `tone_secs` of a 220 Hz tone, then `silence_secs` of digital silence.
fn wav(amplitude: f32, tone_secs: usize, silence_secs: usize) -> Vec<u8> {
    let rate = 24_000usize;
    let mut samples = Vec::new();
    for i in 0..rate * tone_secs {
        let t = i as f32 / rate as f32;
        samples.push((amplitude * (t * 220.0 * std::f32::consts::TAU).sin() * 32767.0) as i16);
    }
    samples.extend(std::iter::repeat_n(0i16, rate * silence_secs));
    let data_len = samples.len() * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&u32::try_from(36 + data_len).unwrap().to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(rate).unwrap().to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(rate * 2).unwrap().to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&u32::try_from(data_len).unwrap().to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kuleshov-analyze-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_retail_encode_has_the_requested_format_lead_in_and_noise_floor() {
    let dir = workspace("retail");
    let input = dir.join("speech.wav");
    fs::write(&input, wav(0.1, 3, 2)).unwrap();
    let output = combine_audio_files_with_options(
        &tools(),
        &[input],
        44_100,
        &dir,
        &CombineOptions {
            encoding: Mp3Encoding::Cbr(192),
            room_tone: Some(RoomTone {
                lead_secs: 1.5,
                level_dbfs: -70.0,
            }),
            tags: Vec::new(),
        },
        None,
    )
    .unwrap();
    let report = analyze_audio(&tools(), &output).unwrap();

    assert_eq!(report.codec, "mp3");
    assert_eq!(report.sample_rate, 44_100);
    assert_eq!(report.channels, 1);
    assert_eq!(report.bit_rate, Some(192_000));
    assert!(
        (1.4..1.7).contains(&report.lead_secs),
        "lead {}",
        report.lead_secs
    );
    assert!(
        (1.8..2.3).contains(&report.tail_secs),
        "tail {}",
        report.tail_secs
    );
    assert!(
        (-76.0..-64.0).contains(&report.noise_floor_dbfs),
        "noise floor {}",
        report.noise_floor_dbfs
    );
    // 0.1 amplitude is -20 dBFS peak; the tone is -23 dBFS RMS over its 3 s of 6.5 s.
    assert!(
        (-21.0..-19.0).contains(&report.peak_dbfs),
        "peak {}",
        report.peak_dbfs
    );
    assert!(
        (6.0..7.5).contains(&report.duration_secs),
        "{}",
        report.duration_secs
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn without_room_tone_the_silence_stays_digital_and_the_default_encoding_is_vbr() {
    let dir = workspace("plain");
    let input = dir.join("speech.wav");
    fs::write(&input, wav(0.1, 3, 2)).unwrap();
    let output = combine_audio_files_with_options(
        &tools(),
        &[input],
        44_100,
        &dir,
        &CombineOptions::default(),
        None,
    )
    .unwrap();
    let report = analyze_audio(&tools(), &output).unwrap();
    assert!(report.lead_secs < 0.1, "lead {}", report.lead_secs);
    assert!(
        report.noise_floor_dbfs < -90.0,
        "noise floor {}",
        report.noise_floor_dbfs
    );
    // LAME marks a variable-bit-rate file with a "Xing" header, a constant one with "Info".
    let bytes = fs::read(&output).unwrap();
    let marked = |tag: &[u8]| bytes.windows(4).take(4_096).any(|window| window == tag);
    assert!(
        marked(b"Xing") && !marked(b"Info"),
        "the default encoding is not VBR"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_meter_finds_sound_and_silence_in_raw_samples() {
    let mut meter = Meter::default();
    for i in 0..RATE {
        meter.push(if i < RATE / 2 { 0.0 } else { 0.5 });
    }
    assert_eq!(meter.count as usize, RATE);
    assert_eq!(meter.first_sound, Some(25));
    assert!((meter.peak - 0.5).abs() < 1e-6);
    assert_eq!(dbfs(meter.quietest_window.unwrap()), FLOOR_DBFS);
}

#[test]
fn tags_are_written_into_the_mp3() {
    let dir = workspace("tags");
    let input = dir.join("speech.wav");
    fs::write(&input, wav(0.1, 1, 0)).unwrap();
    let options = CombineOptions {
        tags: vec![
            ("comment".into(), "AI-generated narration".into()),
            ("AIGC".into(), r#"{"Label":"1"}"#.into()),
        ],
        ..CombineOptions::default()
    };
    let output =
        combine_audio_files_with_options(&tools(), &[input], 44_100, &dir, &options, None).unwrap();
    let probe = tools()
        .command(crate::Tool::Ffprobe)
        .args(["-v", "error", "-show_entries", "format_tags", "-of", "json"])
        .arg(&output)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&probe.stdout);
    assert!(text.contains("AI-generated narration"), "{text}");
    assert!(text.contains("AIGC"), "{text}");
    assert!(text.contains(r#"{\"Label\":\"1\"}"#), "{text}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn audio_is_combined_in_a_folder_with_chinese_characters_and_apostrophes() {
    let dir = std::env::temp_dir().join(format!("kuleshov-中文-张伟-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // An apostrophe too: `O'Brien` and `L'été` are ordinary folder names.
    let inputs = dir.join("张伟的文档").join("O'Brien 的书 L'été");
    fs::create_dir_all(&inputs).unwrap();
    let first = inputs.join("语音一.wav");
    let second = inputs.join("语音二.wav");
    fs::write(&first, wav(0.1, 1, 0)).unwrap();
    fs::write(&second, wav(0.1, 1, 0)).unwrap();

    let output = combine_audio_files_with_options(
        &tools(),
        &[first, second],
        44_100,
        &inputs,
        &CombineOptions::default(),
        None,
    )
    .unwrap();

    let analysis = analyze_audio(&tools(), &output).unwrap();
    assert!(
        analysis.duration_secs > 1.8,
        "both files are in the result: {} s",
        analysis.duration_secs
    );
    let _ = fs::remove_dir_all(dir);
}
