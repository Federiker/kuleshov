//! What the unit tests share: the tools they run.

use crate::{Tool, Tools};
use std::path::PathBuf;

/// The FFmpeg the tests run: the folder `KULESHOV_TOOLS_DIR` names (mise.toml sets it to the
/// pinned build `mise run tools` stages), else `PATH`.
///
/// # Panics
/// When that folder lacks ffmpeg or ffprobe, naming it.
pub(crate) fn tools() -> Tools {
    let Some(dir) = std::env::var_os("KULESHOV_TOOLS_DIR").map(PathBuf::from) else {
        return Tools::from_path();
    };
    for tool in [Tool::Ffmpeg, Tool::Ffprobe] {
        assert!(
            dir.join(tool.file_name()).is_file(),
            "KULESHOV_TOOLS_DIR={} holds no {tool}: run `mise run tools`, or point it at a \
folder with ffmpeg and ffprobe",
            dir.display()
        );
    }
    Tools::find_in([dir])
}
