//! Integration tests for the video helpers. They need `ffmpeg` and `ffprobe`: the folder
//! `KULESHOV_TOOLS_DIR` names, else `PATH`.
#![expect(
    clippy::unwrap_used,
    reason = "the helpers are part of the tests: a failure is a panic"
)]

use kuleshov::{
    Direction, Tool, Tools, add_audio_to_video, combine_video_files,
    concat_matching_video_files_with_cancel, make_motion_from_still, mux_audio_with_cancel,
    video_duration,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const W: u32 = 90;
const H: u32 = 160;
const FPS: u32 = 10;

/// The tools the tests run, as the unit tests find them (`tests/unit/support.rs`).
fn tools() -> Tools {
    let Some(dir) = std::env::var_os("KULESHOV_TOOLS_DIR").map(PathBuf::from) else {
        return Tools::from_path();
    };
    for tool in [Tool::Ffmpeg, Tool::Ffprobe] {
        assert!(
            dir.join(tool.file_name()).is_file(),
            "KULESHOV_TOOLS_DIR={} holds no {tool}: run `mise run tools`",
            dir.display()
        );
    }
    Tools::find_in([dir])
}

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("kuleshov-videos-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// A 640 by 480 still with gradients and a marker, standing in for manuzio's sample photo.
    fn image(&self) -> PathBuf {
        let (width, height) = (640_u32, 480_u32);
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        for y in 0..height {
            for x in 0..width {
                let marker = (200..260).contains(&x) && (150..210).contains(&y);
                let red = if marker {
                    255
                } else {
                    u8::try_from(x * 255 / width).unwrap()
                };
                let (green, blue) = (y * 255 / height, (x + y) % 256);
                bytes.extend([
                    red,
                    u8::try_from(green).unwrap(),
                    u8::try_from(blue).unwrap(),
                ]);
            }
        }
        let target = self.0.join("still.ppm");
        fs::write(&target, bytes).unwrap();
        target
    }

    fn clip(&self, seconds: f64) -> PathBuf {
        make_motion_from_still(
            &tools(),
            &self.image(),
            W,
            H,
            seconds,
            Direction::ZoomIn,
            None,
            FPS,
            &self.0,
        )
        .unwrap()
    }

    fn tone(&self, seconds: f64) -> PathBuf {
        let path = self.0.join(format!("tone-{seconds}.wav"));
        let status = tools()
            .command(Tool::Ffmpeg)
            .args(["-y", "-f", "lavfi", "-i"])
            .arg(format!("sine=frequency=330:duration={seconds}"))
            .arg(&path)
            .output()
            .unwrap()
            .status;
        assert!(status.success());
        path
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn probe(path: &Path, stream: &str, entries: &str) -> String {
    let out = tools()
        .command(Tool::Ffprobe)
        .args(["-v", "error", "-select_streams", stream])
        .args(["-show_entries", entries, "-of", "csv=p=0"])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn size(path: &Path) -> String {
    probe(path, "v:0", "stream=width,height")
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected about {expected}s, got {actual}s"
    );
}

#[test]
fn every_direction_renders_a_silent_clip_of_the_requested_size_and_length() {
    let ws = Workspace::new();
    let image = ws.image();

    for direction in [
        Direction::ZoomIn,
        Direction::ZoomOut,
        Direction::LeftToRight,
        Direction::RightToLeft,
    ] {
        let clip = make_motion_from_still(&tools(), &image, W, H, 1.0, direction, None, FPS, &ws.0)
            .unwrap();

        assert_eq!(size(&clip), format!("{W},{H}"), "{direction:?}");
        assert_close(video_duration(&tools(), &clip).unwrap(), 1.0, 0.3);
        assert_eq!(probe(&clip, "a", "stream=codec_name"), "");
        assert_eq!(
            probe(&clip, "v:0", "stream=r_frame_rate,nb_frames"),
            "10/1,10"
        );
    }
}

#[test]
fn pans_have_a_distinct_frame_at_each_output_tick_for_all_image_aspect_ratios() {
    let ws = Workspace::new();
    for (width, height) in [(160, 90), (90, 160), (60, 160)] {
        let image = ws.0.join(format!("pattern-{width}-{height}.ppm"));
        let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
        for y in 0..height {
            for x in 0..width {
                let value = if (x / 7 + y / 11) % 2 == 0 { 240 } else { 20 };
                bytes.extend([value; 3]);
            }
        }
        fs::write(&image, bytes).unwrap();
        for direction in [Direction::LeftToRight, Direction::RightToLeft] {
            let video =
                make_motion_from_still(&tools(), &image, W, H, 1.0, direction, None, 30, &ws.0)
                    .unwrap();
            let decoded = tools()
                .command(Tool::Ffmpeg)
                .args(["-v", "error", "-i"])
                .arg(&video)
                .args(["-f", "rawvideo", "-pix_fmt", "gray", "pipe:1"])
                .output()
                .unwrap();
            assert!(decoded.status.success());
            let frames: Vec<_> = decoded.stdout.chunks_exact((W * H) as usize).collect();
            assert_eq!(frames.len(), 30);
            assert!(
                frames.windows(2).all(|w| w[0] != w[1]),
                "{width}x{height}, {direction:?}: every frame must advance the motion"
            );
        }
    }
}
#[test]
fn clips_get_unique_names_inside_a_created_output_folder() {
    let ws = Workspace::new();
    let nested = ws.0.join("nested/deeper");

    let a = make_motion_from_still(
        &tools(),
        &ws.image(),
        W,
        H,
        0.5,
        Direction::ZoomIn,
        None,
        FPS,
        &nested,
    )
    .unwrap();
    let b = make_motion_from_still(
        &tools(),
        &ws.image(),
        W,
        H,
        0.5,
        Direction::ZoomIn,
        None,
        FPS,
        &nested,
    )
    .unwrap();

    assert_ne!(a, b);
    assert!(a.starts_with(&nested) && a.extension().is_some_and(|e| e == "mp4"));
}

#[test]
fn very_short_durations_are_clamped_to_a_playable_clip() {
    let ws = Workspace::new();

    let clip = make_motion_from_still(
        &tools(),
        &ws.image(),
        W,
        H,
        0.0,
        Direction::LeftToRight,
        None,
        FPS,
        &ws.0,
    )
    .unwrap();

    // Clamped to 0.1 s: one frame at 10 frames a second.
    assert_close(video_duration(&tools(), &clip).unwrap(), 0.1, 0.05);
}

#[test]
fn a_missing_image_is_reported_as_an_error() {
    let ws = Workspace::new();

    let result = make_motion_from_still(
        &tools(),
        &ws.0.join("missing.jpg"),
        W,
        H,
        1.0,
        Direction::ZoomIn,
        None,
        FPS,
        &ws.0,
    );

    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains("missing.jpg"), "{error}");
}

#[test]
fn combining_without_clips_fails_before_running_ffmpeg() {
    let ws = Workspace::new();

    let error = combine_video_files(&tools(), &[], W, H, 0, FPS, &ws.0).unwrap_err();

    assert!(error.to_string().contains("No input files"));
}

#[test]
fn combining_clips_sums_their_durations_and_normalises_the_size() {
    let ws = Workspace::new();
    let clips = [ws.clip(1.0), ws.clip(1.0)];

    let combined = combine_video_files(&tools(), &clips, W, H, 0, FPS, &ws.0).unwrap();

    assert_close(video_duration(&tools(), &combined).unwrap(), 2.0, 0.4);
    assert_eq!(size(&combined), format!("{W},{H}"));
}

#[test]
fn combining_with_a_fade_keeps_the_total_length() {
    let ws = Workspace::new();
    let clips = [ws.clip(1.0), ws.clip(1.0), ws.clip(1.0)];

    let combined = combine_video_files(&tools(), &clips, W, H, 300, FPS, &ws.0).unwrap();

    assert_close(video_duration(&tools(), &combined).unwrap(), 3.0, 0.5);
}

#[test]
fn combining_rejects_a_missing_clip() {
    let ws = Workspace::new();

    let result = combine_video_files(&tools(), &[ws.0.join("missing.mp4")], W, H, 0, FPS, &ws.0);

    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains("missing.mp4"), "{error}");
}

#[test]
fn matching_clips_are_copied_with_exact_frame_counts_and_monotonic_timestamps() {
    let ws = Workspace::new();
    let originals = [ws.clip(1.0), ws.clip(1.5)];
    let clips = [
        ws.0.join("first clip's.mp4"),
        ws.0.join("second clip's.mp4"),
    ];
    for (original, target) in originals.iter().zip(&clips) {
        fs::rename(original, target).unwrap();
    }
    let joined = concat_matching_video_files_with_cancel(&tools(), &clips, W, H, FPS, &ws.0, None)
        .unwrap()
        .expect("matching clips should use stream copy");
    assert_eq!(
        probe(&joined, "v:0", "stream=r_frame_rate,nb_frames"),
        "10/1,25"
    );
    assert_close(video_duration(&tools(), &joined).unwrap(), 2.5, 0.01);
    let times = probe(&joined, "v:0", "frame=best_effort_timestamp_time");
    let pts: Vec<f64> = times
        .lines()
        .filter_map(|line| line.trim_end_matches(',').parse().ok())
        .collect();
    assert_eq!(pts.len(), 25);
    assert!(pts.windows(2).all(|w| (w[1] - w[0] - 0.1).abs() < 0.001));
    assert!(
        concat_matching_video_files_with_cancel(&tools(), &clips, W * 2, H, FPS, &ws.0, None)
            .unwrap()
            .is_none()
    );
    assert!(
        concat_matching_video_files_with_cancel(
            &tools(),
            &[ws.0.join("missing.mp4")],
            W,
            H,
            FPS,
            &ws.0,
            None
        )
        .is_err()
    );
}

fn video_hash(path: &Path) -> Vec<u8> {
    let output = tools()
        .command(Tool::Ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-map", "0:v:0", "-c:v", "copy", "-f", "hash", "-hash", "sha256", "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}

#[test]
fn copy_mux_preserves_video_and_trims_shorter_audio_or_video() {
    let ws = Workspace::new();
    let clip = ws.clip(1.0);
    let muxed = mux_audio_with_cancel(&tools(), &clip, &ws.tone(2.0), None, &ws.0, None).unwrap();
    assert_eq!(video_hash(&clip), video_hash(&muxed));
    assert_eq!(
        probe(&muxed, "v:0", "stream=r_frame_rate,nb_frames"),
        "10/1,10"
    );
    assert_eq!(probe(&muxed, "a:0", "stream=codec_name"), "aac");
    assert_close(video_duration(&tools(), &muxed).unwrap(), 1.0, 0.15);
    let shorter = mux_audio_with_cancel(&tools(), &clip, &ws.tone(0.5), None, &ws.0, None).unwrap();
    assert_close(video_duration(&tools(), &shorter).unwrap(), 0.5, 0.15);
}

#[test]
fn cancelled_copy_operations_remove_partial_outputs() {
    let ws = Workspace::new();
    let clip = ws.clip(1.0);
    let tone = ws.tone(1.0);
    let cancel = std::sync::atomic::AtomicBool::new(true);
    let out = ws.0.join("cancelled");
    assert!(mux_audio_with_cancel(&tools(), &clip, &tone, None, &out, Some(&cancel)).is_err());
    assert!(fs::read_dir(&out).unwrap().next().is_none());
    assert!(
        concat_matching_video_files_with_cancel(&tools(), &[clip], W, H, FPS, &out, Some(&cancel))
            .is_err()
    );
    assert!(fs::read_dir(&out).unwrap().next().is_none());
}

#[test]
fn frame_aligned_copy_mux_preserves_every_frame_and_rejects_invalid_durations() {
    let ws = Workspace::new();
    let clip = make_motion_from_still(
        &tools(),
        &ws.image(),
        W,
        H,
        4.0,
        Direction::ZoomIn,
        None,
        30,
        &ws.0,
    )
    .unwrap();
    let tone = ws.tone(4.0);
    for _ in 0..3 {
        let muxed = mux_audio_with_cancel(&tools(), &clip, &tone, Some(4.0), &ws.0, None).unwrap();
        assert_eq!(
            probe(&muxed, "v:0", "stream=r_frame_rate,nb_frames"),
            "30/1,120"
        );
        assert_eq!(video_hash(&clip), video_hash(&muxed));
        assert_close(video_duration(&tools(), &muxed).unwrap(), 4.0, 0.01);
    }
    for duration in [0.0, f64::NAN, f64::INFINITY] {
        assert!(
            mux_audio_with_cancel(&tools(), &clip, &tone, Some(duration), &ws.0, None).is_err()
        );
    }
}

#[test]
fn fades_report_bad_clip_duration_instead_of_panicking() {
    let ws = Workspace::new();
    // A file that exists but is no video: its duration cannot be read for the fade.
    let broken = ws.0.join("broken.mp4");
    fs::write(&broken, b"not a video").unwrap();
    let clips = [broken, ws.clip(1.0)];
    let error = format!(
        "{:#}",
        combine_video_files(&tools(), &clips, W, H, 300, FPS, &ws.0).unwrap_err()
    );
    assert!(
        error.contains("ffprobe could not read") && error.contains("broken.mp4"),
        "{error}"
    );
}

#[test]
fn adding_audio_trims_to_the_shorter_input() {
    let ws = Workspace::new();
    let video = ws.clip(2.0);
    let audio = ws.tone(1.0);

    let muxed = add_audio_to_video(&tools(), &video, &audio, Some(FPS), &ws.0).unwrap();

    assert_eq!(probe(&muxed, "a", "stream=codec_name"), "aac");
    assert_close(video_duration(&tools(), &muxed).unwrap(), 1.0, 0.4);
}

#[test]
fn adding_audio_requires_an_audio_file() {
    let ws = Workspace::new();

    let result = add_audio_to_video(
        &tools(),
        &ws.clip(1.0),
        &ws.0.join("missing.wav"),
        None,
        &ws.0,
    );

    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains("missing.wav"), "{error}");
}

#[test]
fn duration_of_a_missing_file_is_an_error() {
    let ws = Workspace::new();

    let error = video_duration(&tools(), &ws.0.join("nothing.mp4")).unwrap_err();
    assert!(
        error.to_string().contains("ffprobe could not read")
            && error.to_string().contains("nothing.mp4"),
        "{error:#}"
    );
}
