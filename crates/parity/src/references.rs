use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/// One platform's references: every scenario's tool calls, result, outputs and leftover files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// What recorded it, such as "manuzio 0ec8e8e" (never compared).
    pub source: String,
    /// First line of `ffmpeg -version`: the outputs belong to this build.
    pub ffmpeg: String,
    /// First line of `ffprobe -version`.
    pub ffprobe: String,
    pub scenarios: Vec<Scenario>,
}

/// One call of a public function, as the tools saw it and as it left the disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    /// Every `ffmpeg` and `ffprobe` run, in order.
    pub calls: Vec<Call>,
    pub result: Value,
    /// The files the function returned, in order.
    pub outputs: Vec<Output>,
    /// Every file left in the scenario's folder afterwards, sorted: no concat list, no partial output.
    pub files: Vec<String>,
}

/// One run of a tool, logged by the recording shim. Paths are normalised (docs/parity.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Call {
    /// `ffmpeg` or `ffprobe`.
    pub tool: String,
    pub args: Vec<String>,
    /// The contents of the list files among the arguments (FFmpeg's concat lists), by argument.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lists: BTreeMap<String, String>,
}

/// What a function returned: a value written as text, or an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Value {
    Ok(String),
    /// The message, kept for reading; the parity test only asks that the function failed too.
    Err(String),
}

/// What a scenario's function gave back, before normalisation.
#[derive(Debug, Default)]
pub struct Outcome {
    pub value: Option<Result<String, String>>,
    pub outputs: Vec<PathBuf>,
}

impl Outcome {
    /// A function that returned one file.
    pub fn file<E: std::fmt::Display>(result: Result<PathBuf, E>) -> Self {
        match result {
            Ok(path) => Self {
                value: Some(Ok(String::new())),
                outputs: vec![path],
            },
            Err(error) => Self::error(&error),
        }
    }

    /// A function that returned a value other than a file.
    pub fn value<T: std::fmt::Debug, E: std::fmt::Display>(result: Result<T, E>) -> Self {
        match result {
            Ok(value) => Self {
                value: Some(Ok(format!("{value:?}"))),
                outputs: Vec::new(),
            },
            Err(error) => Self::error(&error),
        }
    }

    /// A function that failed; `{:#}` keeps an `anyhow` chain.
    pub fn error<E: std::fmt::Display>(error: &E) -> Self {
        Self {
            value: Some(Err(format!("{error:#}"))),
            outputs: Vec::new(),
        }
    }
}

/// A file a function wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    /// Its normalised path.
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    /// `-f md5` of the first audio stream, decoded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_md5: Option<String>,
    /// SHA-256 of the `-f framemd5` listing of the first video stream, decoded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_framemd5: Option<String>,
    /// The frames in that listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u64>,
    /// ffprobe's stream and format parameters, one line each.
    pub streams: Vec<String>,
}

/// The name of this platform's reference file in `references/`.
#[must_use]
pub fn platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    }
}
