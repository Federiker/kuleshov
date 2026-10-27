//! Drives FFmpeg and FFprobe for audio and video: durations, concatenation into MP3 or WAV with
//! room tone and tags, sample-exact slots, level measurements, motion from still images, joining
//! clips and muxing audio.
//!
//! The caller says where `ffmpeg` and `ffprobe` are with [`Tools`]; kuleshov never bundles or
//! searches for them, and downloads them only when asked (the `download` feature,
//! `Tools::find_or_download`). Every function that runs them takes `&Tools` first.

mod audio;
mod concat;
#[cfg(feature = "download")]
mod download;
mod error;
mod process;
mod tools;
mod video;

pub use audio::{
    AudioAnalysis, CombineOptions, Mp3Encoding, RoomTone, analyze_audio, analyze_audio_with_cancel,
    audio_duration, combine_audio_files, combine_audio_files_with_cancel,
    combine_audio_files_with_options, concat_wav, concat_wav_with_cancel, place_in_slot,
    place_in_slot_with_cancel,
};
#[cfg(feature = "tokio")]
pub use audio::{audio_duration_async, combine_audio_files_async};
#[cfg(feature = "download")]
pub use download::FfmpegBuild;
pub use error::{Error, Result};
pub use process::{run_cancellable, stderr_tail};
pub use tools::{Tool, Tools, hide_console};
pub use video::{
    ClipFades, Direction, add_audio_to_video, add_audio_to_video_with_cancel, combine_video_files,
    combine_video_files_with_cancel, concat_matching_video_files_with_cancel,
    make_faded_motion_from_still_with_cancel, make_motion_from_still,
    make_motion_from_still_with_cancel, mux_audio_with_cancel, video_duration,
};

#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod support;
