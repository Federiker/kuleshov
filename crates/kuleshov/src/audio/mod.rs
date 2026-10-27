//! Audio: duration, concatenation into MP3 or WAV, slot placement and measurements.

mod analyze;
mod combine;
mod duration;
mod slot;

pub use analyze::{AudioAnalysis, analyze_audio};
#[cfg(feature = "tokio")]
pub use combine::combine_audio_files_async;
pub use combine::{
    CombineOptions, Mp3Encoding, RoomTone, combine_audio_files, combine_audio_files_with_cancel,
    combine_audio_files_with_options,
};
pub use duration::audio_duration;
#[cfg(feature = "tokio")]
pub use duration::audio_duration_async;
pub use slot::{concat_wav, concat_wav_with_cancel, place_in_slot, place_in_slot_with_cancel};
