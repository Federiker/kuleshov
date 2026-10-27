use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// A mono 16-bit WAV at `rate`: `tone_secs` of a `frequency` Hz sine at `amplitude` (0 to 1), then
/// `silence_secs` of digital silence. As manuzio's audio tests write it, so the bytes never depend
/// on an FFmpeg build.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "sample arithmetic on small, positive counts"
)]
pub fn wav(
    rate: u32,
    frequency: f32,
    amplitude: f32,
    tone_secs: f64,
    silence_secs: f64,
) -> Vec<u8> {
    let tone = (f64::from(rate) * tone_secs).round() as usize;
    let silence = (f64::from(rate) * silence_secs).round() as usize;
    let mut samples = Vec::with_capacity(tone + silence);
    for i in 0..tone {
        let t = i as f32 / rate as f32;
        samples.push((amplitude * (t * frequency * std::f32::consts::TAU).sin() * 32767.0) as i16);
    }
    samples.extend(std::iter::repeat_n(0i16, silence));
    let data_len = u32::try_from(samples.len() * 2).unwrap_or(u32::MAX);
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// A binary PPM of `width` by `height`: a chequerboard of 7 by 11 pixel cells, as manuzio's pan
/// test draws, with a white square at a third of the width and height so motion is visible.
#[must_use]
pub fn ppm(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    let (left, top) = (width / 3, height / 3);
    let side = (width.min(height) / 8).max(2);
    for y in 0..height {
        for x in 0..width {
            let marker = (left..left + side).contains(&x) && (top..top + side).contains(&y);
            let value = if marker {
                255
            } else if (x / 7 + y / 11).is_multiple_of(2) {
                200
            } else {
                40
            };
            // A little colour, so chroma takes part in the encode.
            bytes.extend([value, value / 2 + (x % 64) as u8, 255 - value]);
        }
    }
    bytes
}

/// The input files every scenario reads, written into one folder.
pub struct Fixtures {
    dir: PathBuf,
}

impl Fixtures {
    /// Write every fixture into `dir`.
    ///
    /// # Errors
    /// When a file cannot be written.
    pub fn write(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let files: [(&str, Vec<u8>); 9] = [
            // As manuzio's retail test: 3 s of tone, 2 s of silence, 24 kHz.
            ("speech.wav", wav(24_000, 220.0, 0.1, 3.0, 2.0)),
            ("tone-1s.wav", wav(24_000, 220.0, 0.1, 1.0, 0.0)),
            ("tone-440-1s.wav", wav(44_100, 440.0, 0.5, 1.0, 0.0)),
            ("tone-440-3s.wav", wav(44_100, 440.0, 0.5, 3.0, 0.0)),
            ("tone-330-half.wav", wav(48_000, 330.0, 0.5, 0.5, 0.0)),
            ("tone-330-2s.wav", wav(48_000, 330.0, 0.5, 2.0, 0.0)),
            ("portrait.ppm", ppm(90, 160)),
            ("landscape.ppm", ppm(160, 90)),
            ("narrow.ppm", ppm(60, 160)),
        ];
        for (name, bytes) in files {
            fs::write(dir.join(name), bytes)?;
        }
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// The path of the fixture `name`.
    #[must_use]
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// The folder holding the fixtures.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}
