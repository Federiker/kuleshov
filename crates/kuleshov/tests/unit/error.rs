use super::*;

fn failed(program: &str, file: Option<&str>, code: Option<i32>, stderr: &str) -> Error {
    Error::Failed {
        program: PathBuf::from(program),
        file: file.map(PathBuf::from),
        code,
        stderr: stderr.to_owned(),
    }
}

#[test]
fn a_failure_names_the_tool_and_the_file() {
    assert_eq!(
        failed(
            "/opt/ffprobe",
            Some("a/nothing.mp4"),
            Some(1),
            "No such file"
        )
        .to_string(),
        format!(
            "ffprobe could not read {}: exited with code 1: No such file",
            Path::new("a/nothing.mp4").display()
        )
    );
    assert_eq!(
        // Forward slashes separate folders on every platform.
        failed("C:/tools/ffmpeg.exe", Some("still.jpg"), None, "").to_string(),
        "ffmpeg failed on still.jpg: was stopped"
    );
    assert_eq!(
        failed("ffmpeg", None, Some(8), "").to_string(),
        "ffmpeg exited with code 8"
    );
}

#[test]
fn a_failure_is_given_its_file_once() {
    let error = failed("ffmpeg", None, Some(1), "").on(Path::new("first.wav"));
    assert_eq!(error.file(), Some(Path::new("first.wav")));
    let error = error.on(Path::new("second.wav"));
    assert_eq!(error.file(), Some(Path::new("first.wav")));
    // Other errors keep what they say.
    assert!(Error::Cancelled.on(Path::new("x")).is_cancelled());
}

#[test]
fn every_error_says_what_and_where() {
    let io = || io::Error::new(io::ErrorKind::PermissionDenied, "denied");
    let cases = [
        (Error::Cancelled, "cancelled", None),
        (
            Error::InvalidInput("no input files provided".to_owned()),
            "no input files provided",
            None,
        ),
        (
            Error::UnsupportedPath {
                path: PathBuf::from("clip.mp4"),
                reason: "no line breaks",
            },
            "clip.mp4: no line breaks",
            Some("clip.mp4"),
        ),
        (
            Error::Probe {
                path: PathBuf::from("a.wav"),
                reason: "holds no audio".to_owned(),
            },
            "a.wav: holds no audio",
            Some("a.wav"),
        ),
        (
            Error::Io {
                path: PathBuf::from("out"),
                source: io(),
            },
            "out: denied",
            Some("out"),
        ),
        (
            Error::Download {
                url: "https://example.org/ff.zip".to_owned(),
                reason: "SHA-256 mismatch".to_owned(),
            },
            "could not install FFmpeg from https://example.org/ff.zip: SHA-256 mismatch",
            None,
        ),
        (
            Error::Run {
                program: PathBuf::from("ffmpeg"),
                source: io(),
            },
            "could not run ffmpeg: denied",
            None,
        ),
    ];
    for (error, message, file) in cases {
        assert_eq!(error.to_string(), message);
        assert_eq!(error.file(), file.map(Path::new));
        assert_eq!(
            std::error::Error::source(&error).is_some(),
            matches!(error, Error::Io { .. } | Error::Run { .. })
        );
        assert_eq!(error.is_cancelled(), matches!(error, Error::Cancelled));
    }
}
