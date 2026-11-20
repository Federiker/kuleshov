use super::*;
use crate::download::support::{Reply, Server, scratch, sha256};
use std::sync::Arc;

fn body() -> Arc<Vec<u8>> {
    // Several reads' worth, so that progress and cancellation have chunks to act between.
    Arc::new((0..300_000_u32).map(|n| (n % 251) as u8).collect())
}

#[test]
fn a_download_is_written_checked_and_reported() {
    let bytes = body();
    let server = Server::new(vec![("ff.zip", bytes.clone())]);
    let dir = scratch("fetch");
    let file = dir.join("ff.download");
    let build = FfmpegBuild::new(server.url("ff.zip"), sha256(&bytes).to_uppercase());
    let mut reports = Vec::new();

    fetch(
        &build,
        &file,
        &mut |done, total| reports.push((done, total)),
        None,
    )
    .unwrap();

    assert_eq!(std::fs::read(&file).unwrap(), *bytes);
    let total = Some(bytes.len() as u64);
    assert_eq!(reports.first(), Some(&(0, total)));
    assert_eq!(reports.last(), Some(&(bytes.len() as u64, total)));
    assert!(reports.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_download_with_other_bytes_is_refused() {
    let bytes = body();
    let server = Server::new(vec![("ff.zip", bytes)]);
    let dir = scratch("checksum");
    let build = FfmpegBuild::new(server.url("ff.zip"), "0".repeat(64));

    let error = fetch(&build, &dir.join("ff.download"), &mut |_, _| {}, None).unwrap_err();

    assert!(matches!(error, Error::Download { .. }), "{error:?}");
    assert!(error.to_string().contains("SHA-256"), "{error}");
    assert!(error.to_string().contains(&server.url("ff.zip")), "{error}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_missing_file_on_the_server_is_an_error_naming_it() {
    let server = Server::new(Vec::new());
    let dir = scratch("missing");
    let build = FfmpegBuild::new(server.url("gone.zip"), "0".repeat(64));

    let error = fetch(&build, &dir.join("ff.download"), &mut |_, _| {}, None).unwrap_err();

    assert!(matches!(error, Error::Download { .. }), "{error:?}");
    assert!(error.to_string().contains("404"), "{error}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_malformed_checksum_or_a_cancelled_download_asks_nothing_of_the_server() {
    let server = Server::new(vec![("ff.zip", body())]);
    let dir = scratch("refused");
    let file = dir.join("ff.download");
    let error = fetch(
        &FfmpegBuild::new(server.url("ff.zip"), "not a checksum"),
        &file,
        &mut |_, _| {},
        None,
    )
    .unwrap_err();
    assert!(matches!(error, Error::InvalidInput(_)), "{error:?}");
    let cancel = AtomicBool::new(true);
    let build = FfmpegBuild::new(server.url("ff.zip"), "0".repeat(64));
    let error = fetch(&build, &file, &mut |_, _| {}, Some(&cancel)).unwrap_err();
    assert!(error.is_cancelled());
    assert_eq!(server.requests(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancelling_stops_between_two_chunks() {
    let bytes = body();
    let server = Server::new(vec![("ff.zip", bytes.clone())]);
    let dir = scratch("cancel");
    let build = FfmpegBuild::new(server.url("ff.zip"), sha256(&bytes));
    let cancel = AtomicBool::new(false);

    let error = fetch(
        &build,
        &dir.join("ff.download"),
        &mut |done, _| {
            if done > 0 {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        },
        Some(&cancel),
    )
    .unwrap_err();

    assert!(error.is_cancelled());
    std::fs::remove_dir_all(dir).unwrap();
}

/// No waiting between attempts, so that the retries take no time.
const NOW: [Duration; 2] = [Duration::ZERO, Duration::ZERO];

#[test]
fn a_passing_failure_is_tried_again_until_the_download_succeeds() {
    let bytes = body();
    let server = Server::scripted(vec![(
        "ff.zip",
        vec![
            Reply::Status(503),
            Reply::Cut(bytes.clone()),
            Reply::Body(bytes.clone()),
        ],
    )]);
    let dir = scratch("retried");
    let file = dir.join("ff.download");
    let build = FfmpegBuild::new(server.url("ff.zip"), sha256(&bytes));
    let mut starts = 0;

    fetch_with(
        &build,
        &file,
        &mut |done, _| starts += usize::from(done == 0),
        None,
        &NOW,
    )
    .unwrap();

    assert_eq!(server.requests(), 3);
    assert_eq!(std::fs::read(&file).unwrap(), *bytes);
    // The cut-off attempt and the last one reported from zero.
    assert_eq!(starts, 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_source_that_keeps_failing_is_tried_three_times_and_named() {
    let server = Server::scripted(vec![("ff.zip", vec![Reply::Status(429)])]);
    let dir = scratch("exhausted");
    let file = dir.join("ff.download");
    let build = FfmpegBuild::new(server.url("ff.zip"), "0".repeat(64));

    let error = fetch_with(&build, &file, &mut |_, _| {}, None, &NOW).unwrap_err();

    assert_eq!(server.requests(), 3);
    let Error::Download { url, reason } = &error else {
        panic!("{error:?}")
    };
    assert_eq!(*url, server.url("ff.zip"));
    assert!(
        reason.contains("429") && reason.ends_with("(after 3 attempts)"),
        "{reason}"
    );
    assert!(!file.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_refusal_or_other_bytes_go_straight_to_the_next_source() {
    let bytes = body();
    let other = Arc::new(b"not the archive".to_vec());
    let server = Server::scripted(vec![
        ("gone.zip", vec![Reply::Status(404)]),
        ("other.zip", vec![Reply::Body(other)]),
        ("ff.zip", vec![Reply::Body(bytes.clone())]),
    ]);
    let dir = scratch("fallback");
    let file = dir.join("ff.download");
    let build = FfmpegBuild::new(server.url("gone.zip"), sha256(&bytes))
        .with_mirrors([server.url("other.zip"), server.url("ff.zip")]);

    fetch_with(&build, &file, &mut |_, _| {}, None, &NOW).unwrap();

    // One request each: neither a 404 nor a wrong checksum is asked again.
    assert_eq!(server.requests(), 3);
    assert_eq!(std::fs::read(&file).unwrap(), *bytes);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn when_every_source_fails_the_error_names_each_with_its_reason() {
    let server = Server::scripted(vec![
        ("first.zip", vec![Reply::Status(500)]),
        ("second.zip", vec![Reply::Status(403)]),
    ]);
    let dir = scratch("all-fail");
    let build = FfmpegBuild::new(server.url("first.zip"), "0".repeat(64))
        .with_mirrors([server.url("second.zip")]);

    let error =
        fetch_with(&build, &dir.join("ff.download"), &mut |_, _| {}, None, &NOW).unwrap_err();

    assert_eq!(server.requests(), 4);
    assert_eq!(
        error.to_string(),
        format!(
            "could not install FFmpeg from {}: http status: 500 (after 3 attempts); also tried {}: http status: 403",
            server.url("first.zip"),
            server.url("second.zip")
        )
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancelling_during_the_wait_between_attempts_stops_at_once() {
    let server = Server::scripted(vec![("ff.zip", vec![Reply::Status(502)])]);
    let dir = scratch("cancel-wait");
    let file = dir.join("ff.download");
    let build =
        FfmpegBuild::new(server.url("ff.zip"), "0".repeat(64)).with_mirrors([server.url("ff.zip")]);
    let cancel = AtomicBool::new(false);
    let started = std::time::Instant::now();

    let error = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(200));
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        fetch_with(
            &build,
            &file,
            &mut |_, _| {},
            Some(&cancel),
            &[Duration::from_mins(1)],
        )
        .unwrap_err()
    });

    assert!(error.is_cancelled(), "{error:?}");
    assert!(started.elapsed() < Duration::from_secs(10));
    // Neither the second attempt nor the mirror was asked.
    assert_eq!(server.requests(), 1);
    assert!(!file.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_default_waits_are_a_second_and_then_three() {
    assert_eq!(BACKOFF, [Duration::from_secs(1), Duration::from_secs(3)]);
    wait(Duration::from_millis(1), None).unwrap();
}

#[test]
fn passing_failures_are_told_from_lasting_ones() {
    for error in [
        ureq::Error::HostNotFound,
        ureq::Error::ConnectionFailed,
        ureq::Error::Timeout(ureq::Timeout::Connect),
        ureq::Error::Io(std::io::Error::other("os error 11002")),
        ureq::Error::StatusCode(429),
        ureq::Error::StatusCode(500),
        ureq::Error::StatusCode(503),
    ] {
        assert!(transient(&error), "{error}");
    }
    for error in [
        ureq::Error::StatusCode(400),
        ureq::Error::StatusCode(404),
        ureq::Error::StatusCode(410),
        ureq::Error::BadUri("nowhere".into()),
        ureq::Error::Tls("refused"),
    ] {
        assert!(!transient(&error), "{error}");
    }
}
