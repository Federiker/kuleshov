use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The pinned builds, shared with `scripts/downloads.ts`, which stages them for the tests.
const MANIFEST: &str = include_str!("../../ffmpeg-builds.json");

/// An FFmpeg build to install: an archive (`.zip` or `.tar.xz`) that holds `ffmpeg` and
/// `ffprobe` at any depth, and its SHA-256.
///
/// The archive is downloaded from `url`, and when that fails from each of `mirrors` in turn:
/// copies of the same archive, with the same SHA-256. A manifest entry or a record without
/// `mirrors` has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FfmpegBuild {
    pub url: String,
    /// Lowercase hexadecimal.
    pub sha256: String,
    /// Other places that serve the same archive, tried in order after `url`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<String>,
}

impl FfmpegBuild {
    /// A build of the caller's choosing, from one place.
    #[must_use]
    pub fn new(url: impl Into<String>, sha256: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            sha256: sha256.into(),
            mirrors: Vec::new(),
        }
    }

    /// The same build, also served by `mirrors`, tried in order after the URL.
    #[must_use]
    pub fn with_mirrors<S: Into<String>>(mut self, mirrors: impl IntoIterator<Item = S>) -> Self {
        self.mirrors.extend(mirrors.into_iter().map(Into::into));
        self
    }

    /// Every place the archive is downloaded from, in the order they are tried: the URL, then the
    /// mirrors.
    pub fn sources(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.url.as_str()).chain(self.mirrors.iter().map(String::as_str))
    }

    /// Whether `other` is the same archive: the same SHA-256, wherever it is served from.
    pub(crate) fn same_archive(&self, other: &Self) -> bool {
        self.sha256.eq_ignore_ascii_case(&other.sha256)
    }

    /// The build pinned for this platform: the one the parity references belong to
    /// (docs/parity.md). `None` where none is pinned, such as macOS.
    #[must_use]
    pub fn pinned() -> Option<Self> {
        Self::pinned_for(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// The build pinned for `os` and `arch`, named as [`std::env::consts`] names them
    /// (`windows`, `linux`; `x86_64`, `aarch64`). Windows on arm64 takes the x64 build, which
    /// it runs through its emulation: the parity references belong to that build, and the
    /// native arm64 builds for Windows are rolling ones, not pinned releases.
    #[must_use]
    pub fn pinned_for(os: &str, arch: &str) -> Option<Self> {
        let arch = if (os, arch) == ("windows", "aarch64") {
            "x86_64"
        } else {
            arch
        };
        let builds: BTreeMap<String, serde_json::Value> = serde_json::from_str(MANIFEST).ok()?;
        serde_json::from_value(builds.get(&format!("{os}-{arch}"))?.clone()).ok()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/download/builds.rs"]
mod tests;
