use super::*;
use crate::download::support::{Server, scratch, sha256};
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
