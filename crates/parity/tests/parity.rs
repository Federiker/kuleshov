//! The parity test: every public function, run over the inputs of the references recorded from
//! manuzio's crates (`references/`, docs/parity.md), must call ffmpeg and ffprobe with the same
//! arguments, return the same values and write the same files.
//!
//! The scenarios mirror the recording harness one for one: same names, same order, same inputs.
#![expect(
    clippy::unwrap_used,
    reason = "the scenarios are the test: a failure is a panic"
)]

use kuleshov::{
    ClipFades, CombineOptions, Direction, Mp3Encoding, RoomTone, Tools, add_audio_to_video,
    add_audio_to_video_with_cancel, analyze_audio, audio_duration, audio_duration_async,
    combine_audio_files, combine_audio_files_async, combine_audio_files_with_cancel,
    combine_audio_files_with_options, combine_video_files, combine_video_files_with_cancel,
    concat_matching_video_files_with_cancel, concat_wav, concat_wav_with_cancel,
    make_faded_motion_from_still_with_cancel, make_motion_from_still,
    make_motion_from_still_with_cancel, mux_audio_with_cancel, place_in_slot,
    place_in_slot_with_cancel, video_duration,
};
use kuleshov_parity::{Fixtures, Outcome, Reference, Session, compare, platform};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

const W: u32 = 90;
const H: u32 = 160;
const FPS: u32 = 10;

/// This crate's folder: as Cargo sets it when it runs the test, else as it was when it built it
/// (a test binary built on one system and run on another, such as Windows and WSL).
fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

/// The recording shim: where Cargo built it, else next to this test's own `deps` folder.
fn recorder() -> PathBuf {
    let built = PathBuf::from(env!("CARGO_BIN_EXE_kuleshov-record"));
    if built.is_file() {
        return built;
    }
    let exe = std::env::current_exe().unwrap();
    let profile = exe.parent().and_then(Path::parent).unwrap();
    profile.join(format!("kuleshov-record{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn every_function_calls_ffmpeg_and_writes_what_manuzio_did() {
    let references = manifest_dir()
        .join("references")
        .join(format!("{}.json", platform()));
    let expected: Reference = serde_json::from_str(
        &fs::read_to_string(&references)
            .unwrap_or_else(|error| panic!("{}: {error}", references.display())),
    )
    .unwrap();
    let tools = std::env::var_os("KULESHOV_TOOLS_DIR")
        .map(PathBuf::from)
        .expect(
            "KULESHOV_TOOLS_DIR names the folder with the FFmpeg build of the references \
(`mise run tools` stages it)",
        );
    // Without this, a wrong folder fails later as a missing output of an earlier scenario.
    for tool in [kuleshov::Tool::Ffmpeg, kuleshov::Tool::Ffprobe] {
        assert!(
            tools.join(tool.file_name()).is_file(),
            "KULESHOV_TOOLS_DIR={} holds no {tool}; Cargo runs this test from {}, so a relative \
path is read from there: give an absolute one",
            tools.display(),
            manifest_dir().display()
        );
    }
    let work = std::env::temp_dir().join(format!("kuleshov-parity-{}", std::process::id()));
    let mut s = Session::new(&recorder(), &tools, &work).unwrap();
    let t = &Tools::find_in([s.shim()]);
    run(&mut s, t, &work);

    let actual = s.finish("kuleshov").unwrap();
    let problems = compare(&expected, &actual);
    let report = std::env::temp_dir().join(format!("kuleshov-parity-{}.json", platform()));
    fs::write(
        &report,
        serde_json::to_string_pretty(&actual).unwrap() + "\n",
    )
    .unwrap();
    assert!(
        problems.is_empty(),
        "{} differences from {} (this run: {}):\n{}",
        problems.len(),
        references.display(),
        report.display(),
        problems.join("\n")
    );
    let _ = fs::remove_dir_all(&work);
}

#[expect(
    clippy::too_many_lines,
    reason = "one scenario after the other, as recorded"
)]
fn run(s: &mut Session, t: &Tools, work: &Path) {
    let f = Fixtures::write(&work.join("inputs")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let cancelled = AtomicBool::new(true);

    // Audio.
    s.run("audio-duration", |_| {
        Outcome::value(audio_duration(t, &f.path("speech.wav")))
    })
    .unwrap();
    s.run("audio-duration-missing", |dir| {
        Outcome::value(audio_duration(t, &dir.join("chapter-07.wav")))
    })
    .unwrap();
    s.run("audio-duration-async", |_| {
        Outcome::value(runtime.block_on(audio_duration_async(t, f.path("tone-1s.wav"))))
    })
    .unwrap();
    s.run("combine", |dir| {
        Outcome::file(combine_audio_files(
            t,
            &[f.path("speech.wav"), f.path("tone-1s.wav")],
            44_100,
            dir,
        ))
    })
    .unwrap();
    s.run("combine-cancel", |dir| {
        Outcome::file(combine_audio_files_with_cancel(
            t,
            &[f.path("tone-440-1s.wav")],
            22_050,
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("combine-retail", |dir| {
        let options = CombineOptions {
            encoding: Mp3Encoding::Cbr(192),
            room_tone: Some(RoomTone {
                lead_secs: 1.5,
                level_dbfs: -70.0,
            }),
            tags: Vec::new(),
        };
        Outcome::file(combine_audio_files_with_options(
            t,
            &[f.path("speech.wav")],
            44_100,
            dir,
            &options,
            None,
        ))
    })
    .unwrap();
    s.run("combine-tags", |dir| {
        let options = CombineOptions {
            tags: vec![
                ("comment".into(), "AI-generated narration".into()),
                ("AIGC".into(), r#"{"Label":"1"}"#.into()),
            ],
            ..CombineOptions::default()
        };
        Outcome::file(combine_audio_files_with_options(
            t,
            &[f.path("tone-1s.wav")],
            44_100,
            dir,
            &options,
            None,
        ))
    })
    .unwrap();
    s.run("combine-quality", |dir| {
        let options = CombineOptions {
            encoding: Mp3Encoding::Quality(7),
            ..CombineOptions::default()
        };
        Outcome::file(combine_audio_files_with_options(
            t,
            &[f.path("speech.wav"), f.path("tone-330-half.wav")],
            24_000,
            dir,
            &options,
            None,
        ))
    })
    .unwrap();
    s.run("combine-room-tone-vbr", |dir| {
        let options = CombineOptions {
            encoding: Mp3Encoding::Quality(4),
            room_tone: Some(RoomTone {
                lead_secs: 0.75,
                level_dbfs: -65.0,
            }),
            tags: vec![("title".into(), "Chapter 1".into())],
        };
        Outcome::file(combine_audio_files_with_options(
            t,
            &[f.path("tone-1s.wav")],
            24_000,
            dir,
            &options,
            None,
        ))
    })
    .unwrap();
    s.run("combine-chinese", |dir| {
        let inputs = dir.join("张伟的文档").join("O'Brien 的书 L'été");
        fs::create_dir_all(&inputs).unwrap();
        let first = inputs.join("语音一.wav");
        let second = inputs.join("语音二.wav");
        fs::copy(f.path("tone-1s.wav"), &first).unwrap();
        fs::copy(f.path("speech.wav"), &second).unwrap();
        Outcome::file(combine_audio_files_with_options(
            t,
            &[first, second],
            44_100,
            &inputs,
            &CombineOptions::default(),
            None,
        ))
    })
    .unwrap();
    s.run("combine-async", |dir| {
        Outcome::file(runtime.block_on(combine_audio_files_async(
            t,
            vec![f.path("tone-1s.wav"), f.path("speech.wav")],
            44_100,
            dir.to_path_buf(),
        )))
    })
    .unwrap();
    s.run("combine-empty", |dir| {
        Outcome::file(combine_audio_files_with_options(
            t,
            &[],
            44_100,
            dir,
            &CombineOptions::default(),
            None,
        ))
    })
    .unwrap();
    s.run("combine-cancelled", |dir| {
        Outcome::file(combine_audio_files_with_options(
            t,
            &[f.path("tone-1s.wav")],
            44_100,
            dir,
            &CombineOptions::default(),
            Some(&cancelled),
        ))
    })
    .unwrap();
    let retail = s.output("combine-retail", 0);
    s.run("analyze-retail", |_| {
        Outcome::value(analyze_audio(t, &retail))
    })
    .unwrap();
    let plain = s.output("combine", 0);
    s.run("analyze-plain", |_| {
        Outcome::value(analyze_audio(t, &plain))
    })
    .unwrap();
    s.run("analyze-wav", |_| {
        Outcome::value(analyze_audio(t, &f.path("speech.wav")))
    })
    .unwrap();
    s.run("analyze-missing", |dir| {
        Outcome::value(analyze_audio(t, &dir.join("chapter-08.mp3")))
    })
    .unwrap();
    s.run("slot", |dir| {
        Outcome::file(place_in_slot(
            t,
            &f.path("tone-440-1s.wav"),
            0.5,
            2.0,
            24_000,
            dir,
        ))
    })
    .unwrap();
    s.run("slot-overlong", |dir| {
        Outcome::file(place_in_slot(
            t,
            &f.path("tone-440-3s.wav"),
            0.0,
            1.0,
            24_000,
            dir,
        ))
    })
    .unwrap();
    s.run("slot-cancel", |dir| {
        Outcome::file(place_in_slot_with_cancel(
            t,
            &f.path("tone-330-half.wav"),
            0.2,
            1.5,
            24_000,
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("slot-zero", |dir| {
        Outcome::file(place_in_slot(
            t,
            &f.path("tone-1s.wav"),
            0.0,
            0.0,
            24_000,
            dir,
        ))
    })
    .unwrap();
    s.run("slot-cancelled", |dir| {
        Outcome::file(place_in_slot_with_cancel(
            t,
            &f.path("tone-1s.wav"),
            0.0,
            1.0,
            24_000,
            dir,
            Some(&cancelled),
        ))
    })
    .unwrap();
    let slots = [s.output("slot", 0), s.output("slot-overlong", 0)];
    s.run("concat-wav", |dir| {
        Outcome::file(concat_wav(t, &slots, 24_000, dir))
    })
    .unwrap();
    let slots = [s.output("slot-cancel", 0), s.output("slot", 0)];
    s.run("concat-wav-cancel", |dir| {
        Outcome::file(concat_wav_with_cancel(t, &slots, 24_000, dir, None))
    })
    .unwrap();
    s.run("concat-wav-empty", |dir| {
        Outcome::file(concat_wav(t, &[], 24_000, dir))
    })
    .unwrap();
    s.run("concat-wav-cancelled", |dir| {
        Outcome::file(concat_wav_with_cancel(
            t,
            &slots,
            24_000,
            dir,
            Some(&cancelled),
        ))
    })
    .unwrap();

    // Video.
    for (name, direction) in [
        ("motion-zoom-in", Direction::ZoomIn),
        ("motion-zoom-out", Direction::ZoomOut),
        ("motion-left-to-right", Direction::LeftToRight),
        ("motion-right-to-left", Direction::RightToLeft),
    ] {
        s.run(name, |dir| {
            Outcome::file(make_motion_from_still(
                t,
                &f.path("portrait.ppm"),
                W,
                H,
                1.0,
                direction,
                None,
                FPS,
                dir,
            ))
        })
        .unwrap();
    }
    s.run("motion-landscape-pan", |dir| {
        Outcome::file(make_motion_from_still(
            t,
            &f.path("landscape.ppm"),
            W,
            H,
            1.0,
            Direction::LeftToRight,
            Some(1.3),
            30,
            dir,
        ))
    })
    .unwrap();
    s.run("motion-narrow-cancel", |dir| {
        Outcome::file(make_motion_from_still_with_cancel(
            t,
            &f.path("narrow.ppm"),
            W,
            H,
            1.0,
            Direction::ZoomOut,
            None,
            30,
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("motion-short", |dir| {
        Outcome::file(make_motion_from_still(
            t,
            &f.path("portrait.ppm"),
            W,
            H,
            0.0,
            Direction::RightToLeft,
            None,
            FPS,
            dir,
        ))
    })
    .unwrap();
    s.run("motion-invalid", |dir| {
        Outcome::file(make_motion_from_still(
            t,
            &f.path("portrait.ppm"),
            91,
            H,
            1.0,
            Direction::ZoomIn,
            None,
            FPS,
            dir,
        ))
    })
    .unwrap();
    s.run("motion-missing", |dir| {
        Outcome::file(make_motion_from_still(
            t,
            &dir.join("missing.jpg"),
            W,
            H,
            1.0,
            Direction::ZoomIn,
            None,
            FPS,
            dir,
        ))
    })
    .unwrap();
    s.run("motion-cancelled", |dir| {
        Outcome::file(make_motion_from_still_with_cancel(
            t,
            &f.path("portrait.ppm"),
            W,
            H,
            1.0,
            Direction::ZoomIn,
            None,
            FPS,
            dir,
            Some(&cancelled),
        ))
    })
    .unwrap();
    s.run("faded", |dir| {
        Outcome::file(make_faded_motion_from_still_with_cancel(
            t,
            &f.path("portrait.ppm"),
            W,
            H,
            1.0,
            Direction::ZoomIn,
            ClipFades {
                fade_in: 0.3,
                fade_out: 0.4,
            },
            FPS,
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("faded-none", |dir| {
        Outcome::file(make_faded_motion_from_still_with_cancel(
            t,
            &f.path("landscape.ppm"),
            W,
            H,
            1.5,
            Direction::RightToLeft,
            ClipFades {
                fade_in: 0.0,
                fade_out: 0.0,
            },
            FPS,
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("faded-full-hd", |dir| {
        Outcome::file(make_faded_motion_from_still_with_cancel(
            t,
            &f.path("landscape.ppm"),
            1080,
            1920,
            0.5,
            Direction::ZoomOut,
            ClipFades {
                fade_in: 0.1,
                fade_out: 0.2,
            },
            30,
            dir,
            None,
        ))
    })
    .unwrap();
    let clip = s.output("motion-zoom-in", 0);
    s.run("video-duration", |_| {
        Outcome::value(video_duration(t, &clip))
    })
    .unwrap();
    s.run("video-duration-missing", |dir| {
        Outcome::value(video_duration(t, &dir.join("nothing.mp4")))
    })
    .unwrap();
    let clips = [
        s.output("motion-zoom-in", 0),
        s.output("motion-zoom-out", 0),
    ];
    s.run("combine-video", |dir| {
        Outcome::file(combine_video_files(t, &clips, W, H, 0, FPS, dir))
    })
    .unwrap();
    let clips = [
        s.output("motion-left-to-right", 0),
        s.output("motion-right-to-left", 0),
        s.output("motion-landscape-pan", 0),
    ];
    s.run("combine-video-fade", |dir| {
        Outcome::file(combine_video_files_with_cancel(
            t, &clips, W, H, 300, FPS, dir, None,
        ))
    })
    .unwrap();
    s.run("combine-video-empty", |dir| {
        Outcome::file(combine_video_files(t, &[], W, H, 0, FPS, dir))
    })
    .unwrap();
    let clip = s.output("motion-zoom-in", 0);
    s.run("combine-video-broken", |dir| {
        let broken = dir.join("broken.mp4");
        fs::write(&broken, b"not a video").unwrap();
        Outcome::file(combine_video_files(
            t,
            &[broken, clip.clone()],
            W,
            H,
            300,
            FPS,
            dir,
        ))
    })
    .unwrap();
    s.run("combine-video-cancelled", |dir| {
        Outcome::file(combine_video_files_with_cancel(
            t,
            std::slice::from_ref(&clip),
            W,
            H,
            0,
            FPS,
            dir,
            Some(&cancelled),
        ))
    })
    .unwrap();
    let originals = [s.output("faded", 0), s.output("faded-none", 0)];
    s.run("concat-matching", |dir| {
        let clips = [dir.join("first clip's.mp4"), dir.join("second clip's.mp4")];
        for (original, copy) in originals.iter().zip(&clips) {
            fs::copy(original, copy).unwrap();
        }
        let result = concat_matching_video_files_with_cancel(t, &clips, W, H, FPS, dir, None);
        match result {
            Ok(Some(path)) => Outcome::file(Ok::<_, String>(path)),
            other => Outcome::value(other),
        }
    })
    .unwrap();
    s.run("concat-matching-size", |dir| {
        Outcome::value(concat_matching_video_files_with_cancel(
            t,
            &originals,
            W * 2,
            H,
            FPS,
            dir,
            None,
        ))
    })
    .unwrap();
    let different = [s.output("faded", 0), s.output("motion-zoom-in", 0)];
    s.run("concat-matching-different", |dir| {
        Outcome::value(concat_matching_video_files_with_cancel(
            t, &different, W, H, FPS, dir, None,
        ))
    })
    .unwrap();
    s.run("concat-matching-missing", |dir| {
        Outcome::value(concat_matching_video_files_with_cancel(
            t,
            &[dir.join("missing.mp4")],
            W,
            H,
            FPS,
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("concat-matching-cancelled", |dir| {
        Outcome::value(concat_matching_video_files_with_cancel(
            t,
            &originals,
            W,
            H,
            FPS,
            dir,
            Some(&cancelled),
        ))
    })
    .unwrap();
    let faded = s.output("faded", 0);
    s.run("mux-copy", |dir| {
        Outcome::file(mux_audio_with_cancel(
            t,
            &faded,
            &f.path("tone-330-2s.wav"),
            None,
            dir,
            None,
        ))
    })
    .unwrap();
    let joined = s.output("concat-matching", 0);
    s.run("mux-copy-duration", |dir| {
        Outcome::file(mux_audio_with_cancel(
            t,
            &joined,
            &f.path("tone-330-2s.wav"),
            Some(2.0),
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("mux-copy-invalid", |dir| {
        Outcome::file(mux_audio_with_cancel(
            t,
            &faded,
            &f.path("tone-330-2s.wav"),
            Some(f64::NAN),
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("mux-cancelled", |dir| {
        Outcome::file(mux_audio_with_cancel(
            t,
            &faded,
            &f.path("tone-330-2s.wav"),
            None,
            dir,
            Some(&cancelled),
        ))
    })
    .unwrap();
    let clip = s.output("motion-zoom-in", 0);
    s.run("add-audio", |dir| {
        Outcome::file(add_audio_to_video(
            t,
            &clip,
            &f.path("tone-330-half.wav"),
            Some(FPS),
            dir,
        ))
    })
    .unwrap();
    let combined = s.output("combine-video", 0);
    s.run("add-audio-cancel", |dir| {
        Outcome::file(add_audio_to_video_with_cancel(
            t,
            &combined,
            &f.path("tone-330-2s.wav"),
            None,
            dir,
            None,
        ))
    })
    .unwrap();
    s.run("add-audio-missing", |dir| {
        Outcome::file(add_audio_to_video(
            t,
            &clip,
            &dir.join("missing.wav"),
            None,
            dir,
        ))
    })
    .unwrap();
}
