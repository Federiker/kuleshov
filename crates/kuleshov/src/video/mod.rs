//! Video: motion from a still image, concatenation, audio muxing and duration.

mod audio;
mod combine;
mod duration;
mod ken_burns;

pub use audio::{add_audio_to_video, add_audio_to_video_with_cancel, mux_audio_with_cancel};
pub use combine::{
    combine_video_files, combine_video_files_with_cancel, concat_matching_video_files_with_cancel,
};
pub use duration::video_duration;
pub use ken_burns::{
    ClipFades, Direction, make_faded_motion_from_still_with_cancel, make_motion_from_still,
    make_motion_from_still_with_cancel,
};
