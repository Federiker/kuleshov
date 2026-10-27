use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The pinned builds, shared with `scripts/downloads.ts`, which stages them for the tests.
const MANIFEST: &str = include_str!("../../ffmpeg-builds.json");

/// An FFmpeg build to install: an archive (`.zip` or `.tar.xz`) that holds `ffmpeg` and
/// `ffprobe` at any depth, and its SHA-256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FfmpegBuild {
    pub url: String,
    /// Lowercase hexadecimal.
    pub sha256: String,
}

impl FfmpegBuild {
    /// A build of the caller's choosing.
    #[must_use]
    pub fn new(url: impl Into<String>, sha256: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            sha256: sha256.into(),
        }
    }

    /// The build pinned for this platform: the one the parity references belong to
    /// (docs/parity.md). `None` where none is pinned, such as macOS.
    #[must_use]
    pub fn pinned() -> Option<Self> {
        Self::pinned_for(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// The build pinned for `os` and `arch`, named as [`std::env::consts`] names them
    /// (`windows`, `linux`; `x86_64`, `aarch64`).
    #[must_use]
    pub fn pinned_for(os: &str, arch: &str) -> Option<Self> {
        let builds: BTreeMap<String, serde_json::Value> = serde_json::from_str(MANIFEST).ok()?;
        serde_json::from_value(builds.get(&format!("{os}-{arch}"))?.clone()).ok()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/download/builds.rs"]
mod tests;
