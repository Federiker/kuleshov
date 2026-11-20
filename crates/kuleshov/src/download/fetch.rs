use super::FfmpegBuild;
use crate::{Error, Result, process::cancelled};
use sha2::{Digest, Sha256};
use std::{
    fmt::Write as _,
    fs::{self, File},
    io::{self, Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

/// How long the server may take to accept the connection, and then to answer.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_mins(1);

/// The waits before the second and the third attempt at a source that failed for a passing
/// reason: three attempts in all.
const BACKOFF: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(3)];

/// How often a wait between two attempts looks at the cancel flag.
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// An error about the download of `build`.
pub(crate) fn failed(build: &FfmpegBuild, reason: impl Into<String>) -> Error {
    Error::Download {
        url: build.url.clone(),
        reason: reason.into(),
    }
}

/// Download `build` into `file`, checking its SHA-256 as it arrives: from its URL, then from each
/// mirror, each tried again after [`BACKOFF`] when it fails for a passing reason. `progress` gets
/// the bytes received so far, and the total when the server gives it, from zero at each attempt.
/// The caller removes `file` on an error.
pub(crate) fn fetch(
    build: &FfmpegBuild,
    file: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>),
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    fetch_with(build, file, progress, cancel, &BACKOFF)
}

/// [`fetch`], waiting `backoff` between the attempts at one source (one attempt more than its
/// length).
pub(crate) fn fetch_with(
    build: &FfmpegBuild,
    file: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>),
    cancel: Option<&AtomicBool>,
    backoff: &[Duration],
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
    let mut tried = Vec::new();
    for url in build.sources() {
        let mut attempts = 0;
        let reason = loop {
            attempts += 1;
            let outcome = attempt(&agent, build, url, file, progress, cancel);
            let (reason, again) = match outcome {
                Ok(()) => return Ok(()),
                Err(Attempt::Fatal(error)) => return Err(error),
                Err(Attempt::Permanent(reason)) => (reason, false),
                Err(Attempt::Transient(reason)) => (reason, true),
            };
            remove(file)?;
            match backoff.get(attempts - 1) {
                Some(delay) if again => wait(*delay, cancel)?,
                _ if attempts > 1 => break format!("{reason} (after {attempts} attempts)"),
                _ => break reason,
            }
        };
        tried.push((url, reason));
    }
    let mut tried = tried.into_iter();
    let (_, mut reason) = tried.next().unwrap_or_default();
    for (url, also) in tried {
        let _ = write!(reason, "; also tried {url}: {also}");
    }
    Err(failed(build, reason))
}

/// Why one attempt failed: worth trying the same source again, not worth it, or the end of the
/// whole download (cancelled, or the file cannot be written).
enum Attempt {
    Transient(String),
    Permanent(String),
    Fatal(Error),
}

/// One attempt at downloading `url`, `build`'s archive, into `file`.
fn attempt(
    agent: &ureq::Agent,
    build: &FfmpegBuild,
    url: &str,
    file: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>),
    cancel: Option<&AtomicBool>,
) -> Result<(), Attempt> {
    let mut response = agent.get(url).call().map_err(|error| {
        let reason = error.to_string();
        if transient(&error) {
            Attempt::Transient(reason)
        } else {
            Attempt::Permanent(reason)
        }
    })?;
    let total = response.body().content_length();
    let mut reader = response.body_mut().with_config().limit(u64::MAX).reader();
    let mut out = File::create(file).map_err(|error| Attempt::Fatal(Error::io(file)(error)))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut received = 0u64;
    progress(0, total);
    loop {
        if cancelled(cancel) {
            return Err(Attempt::Fatal(Error::Cancelled));
        }
        // A body cut off on the way is worth asking for again.
        let read = reader
            .read(&mut buffer)
            .map_err(|error| Attempt::Transient(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read])
            .map_err(|error| Attempt::Fatal(Error::io(file)(error)))?;
        received += read as u64;
        progress(received, total);
    }
    out.flush()
        .map_err(|error| Attempt::Fatal(Error::io(file)(error)))?;
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
        Err(Attempt::Permanent(format!(
            "its SHA-256 is {actual}, not the pinned {}",
            build.sha256
        )))
    }
}

/// Whether the request may well succeed if made again: the name or the connection failed, the
/// server was too slow, or it answered 429 (too many requests) or 5xx.
fn transient(error: &ureq::Error) -> bool {
    match error {
        ureq::Error::StatusCode(code) => *code == 429 || (500..=599).contains(code),
        ureq::Error::Io(_)
        | ureq::Error::Timeout(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed => true,
        _ => false,
    }
}

/// Remove what an attempt left in `file`, if anything.
fn remove(file: &Path) -> Result<()> {
    match fs::remove_file(file) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(Error::io(file)(error)),
        _ => Ok(()),
    }
}

/// Wait `delay`, or less when `cancel` turns true.
fn wait(delay: Duration, cancel: Option<&AtomicBool>) -> Result<()> {
    let end = Instant::now() + delay;
    loop {
        if cancelled(cancel) {
            return Err(Error::Cancelled);
        }
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(());
        }
        std::thread::sleep(left.min(CANCEL_POLL));
    }
}

#[cfg(test)]
#[path = "../../tests/unit/download/fetch.rs"]
mod tests;
