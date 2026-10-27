use super::*;
use crate::support::tools;
use std::{fs, path::PathBuf};

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kuleshov-duration-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// One second of a tone, written by FFmpeg.
fn tone(path: &std::path::Path) {
    let status = tools()
        .command(crate::Tool::Ffmpeg)
        .args(["-y", "-v", "error", "-f", "lavfi", "-i", "sine=duration=1"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn a_file_ffprobe_cannot_read_is_named_in_the_error() {
    let dir = workspace("missing");
    let missing = dir.join("chapter-07.wav");

    let error = audio_duration(&tools(), &missing).unwrap_err().to_string();

    assert!(error.contains("chapter-07.wav"), "{error}");
    fs::remove_dir_all(dir).unwrap();
}

/// A file name that is not valid Unicode used to panic (`to_str().unwrap()`).
#[cfg(windows)]
#[test]
fn a_file_name_that_is_not_unicode_is_an_error_not_a_panic() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    let dir = workspace("surrogate");
    // An unpaired surrogate: NTFS accepts it, but FFmpeg reads its arguments as UTF-8 and cannot
    // open it, so the error must say which file.
    let mut name: Vec<u16> = "tone-".encode_utf16().collect();
    name.push(0xD800);
    name.extend(".wav".encode_utf16());
    let path = dir.join(OsString::from_wide(&name));
    tone(&dir.join("tone.wav"));
    fs::rename(dir.join("tone.wav"), &path).unwrap();

    let error = audio_duration(&tools(), &path).unwrap_err().to_string();

    assert!(error.contains("tone-"), "{error}");
    fs::remove_dir_all(dir).unwrap();
}

/// A file name that is not valid UTF-8 used to panic (`to_str().unwrap()`); the bytes reach ffprobe.
#[cfg(unix)]
#[test]
fn a_file_name_that_is_not_unicode_is_read() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let dir = workspace("latin1");
    let path = dir.join(OsStr::from_bytes(b"chapitre-\xe9t\xe9.wav"));
    tone(&path);

    let seconds = audio_duration(&tools(), &path).unwrap();

    assert!((seconds - 1.0).abs() < 0.1, "{seconds}");
    fs::remove_dir_all(dir).unwrap();
}
