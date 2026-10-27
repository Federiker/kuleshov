use super::FfmpegBuild;
use crate::{Error, Result, process::cancelled};
use sha2::{Digest, Sha256};
use std::{
    fmt::Write as _,
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
    time::Duration,
};

/// How long the server may take to accept the connection, and then to answer.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_mins(1);

/// An error about the download of `build`.
pub(crate) fn failed(build: &FfmpegBuild, reason: impl Into<String>) -> Error {
    Error::Download {
        url: build.url.clone(),
        reason: reason.into(),
    }
}

/// Download `build` into `file`, checking its SHA-256 as it arrives. `progress` gets the bytes
/// received so far, and the total when the server gives it. The caller removes `file` on an error.
pub(crate) fn fetch(
    build: &FfmpegBuild,
    file: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>),
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    if build.sha256.len() != 64 || !build.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::InvalidInput(format!(
            "\"{}\" is not a SHA-256 for {}",
            build.sha256, build.url
        )));
    }
    if cancelled(cancel) {
        return Err(Error::Cancelled);
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(&build.url)
        .call()
        .map_err(|error| failed(build, error.to_string()))?;
    let total = response.body().content_length();
    let mut reader = response.body_mut().with_config().limit(u64::MAX).reader();
    let mut out = File::create(file).map_err(Error::io(file))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut received = 0u64;
    progress(0, total);
    loop {
        if cancelled(cancel) {
            return Err(Error::Cancelled);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| failed(build, error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read]).map_err(Error::io(file))?;
        received += read as u64;
        progress(received, total);
    }
    out.flush().map_err(Error::io(file))?;
    let actual = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        });
    if actual.eq_ignore_ascii_case(&build.sha256) {
        Ok(())
    } else {
        Err(failed(
            build,
            format!("its SHA-256 is {actual}, not the pinned {}", build.sha256),
        ))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/download/fetch.rs"]
mod tests;
