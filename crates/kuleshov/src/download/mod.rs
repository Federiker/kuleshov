//! FFmpeg downloaded on request (the `download` feature): the build pinned for this platform, or
//! one the caller names, installed in a folder the caller chooses by [`crate::Tools::find_or_download`].
//! Nothing here runs unless the caller asks: the other functions never download.

mod builds;
mod extract;
mod fetch;
mod install;

pub use builds::FfmpegBuild;

#[cfg(test)]
#[path = "../../tests/unit/download/support.rs"]
mod support;
