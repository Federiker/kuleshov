use super::*;
use crate::download::support::{Server, scratch, sha256, zip};
use std::sync::Arc;

/// A ZIP of the real ffmpeg and ffprobe the tests run (`KULESHOV_TOOLS_DIR`), laid out as the
/// pinned Windows build is, so that the installed pair must really run.
fn real_build() -> Arc<Vec<u8>> {
    let dir = std::env::var_os("KULESHOV_TOOLS_DIR")
        .map(PathBuf::from)
        .expect("KULESHOV_TOOLS_DIR names the folder with ffmpeg and ffprobe");
    let programs = [Tool::Ffmpeg, Tool::Ffprobe].map(|tool| {
        (
            format!("ffmpeg-9/bin/{}", tool.file_name()),
            fs::read(dir.join(tool.file_name())).unwrap(),
        )
    });
    let files: Vec<(&str, &[u8])> = programs
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect();
    Arc::new(zip(&files))
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_build_is_installed_once_kept_while_pinned_and_replaced_when_the_pin_changes() {
    let archive = real_build();
    let checksum = sha256(&archive);
    let server = Server::new(vec![("ff.zip", archive.clone()), ("ff2.zip", archive)]);
    let dir = scratch("install").join("tools");
    let build = FfmpegBuild::new(server.url("ff.zip"), checksum.clone());
    let expected = {
        let mut names = vec![
            RECORD.to_owned(),
            Tool::Ffmpeg.file_name(),
            Tool::Ffprobe.file_name(),
        ];
        names.sort();
        names
    };

    // Missing: downloaded, with progress, into a folder made for it.
    let mut last = (0, None);
    let tools =
        Tools::find_or_download_build(&dir, &build, |done, total| last = (done, total), None)
            .unwrap();
    assert_eq!(server.requests(), 1);
    assert_eq!(last.0, last.1.unwrap());
    assert_eq!(
        tools.path(Tool::Ffprobe),
        dir.join(Tool::Ffprobe.file_name())
    );
    assert_eq!(names(&dir), expected);
    let record: FfmpegBuild =
        serde_json::from_str(&fs::read_to_string(dir.join(RECORD)).unwrap()).unwrap();
    assert_eq!(record, build);
    // The installed pair works for the rest of the library.
    let probe = tools
        .command(Tool::Ffprobe)
        .arg("-version")
        .output()
        .unwrap();
    assert!(probe.status.success());

    // Still the pinned build: kept.
    Tools::find_or_download_build(&dir, &build, |_, _| {}, None).unwrap();
    assert_eq!(server.requests(), 1);

    // Another pin: replaced.
    let other = FfmpegBuild::new(server.url("ff2.zip"), checksum);
    Tools::find_or_download_build(&dir, &other, |_, _| {}, None).unwrap();
    assert_eq!(server.requests(), 2);
    assert_eq!(names(&dir), expected);
    assert!(
        fs::read_to_string(dir.join(RECORD))
            .unwrap()
            .contains("ff2.zip")
    );

    // A pair without a record was put there by hand: kept, whatever the pin.
    fs::remove_file(dir.join(RECORD)).unwrap();
    Tools::find_or_download_build(&dir, &build, |_, _| {}, None).unwrap();
    assert_eq!(server.requests(), 2);

    fs::remove_dir_all(dir.parent().unwrap()).unwrap();
}

#[test]
fn a_failed_or_cancelled_replacement_leaves_the_pair_in_place_and_nothing_else() {
    let server = Server::new(vec![("ff.zip", Arc::new(b"not the build".to_vec()))]);
    let dir = scratch("keep");
    for tool in [Tool::Ffmpeg, Tool::Ffprobe] {
        fs::write(dir.join(tool.file_name()), "old").unwrap();
    }
    let old = FfmpegBuild::new("https://example.org/old.zip", "1".repeat(64));
    let record = serde_json::to_string(&old).unwrap();
    fs::write(dir.join(RECORD), &record).unwrap();
    let before = names(&dir);

    let wrong = FfmpegBuild::new(server.url("ff.zip"), "0".repeat(64));
    let error = Tools::find_or_download_build(&dir, &wrong, |_, _| {}, None).unwrap_err();
    assert!(matches!(error, Error::Download { .. }), "{error:?}");
    let cancel = AtomicBool::new(true);
    let error = Tools::find_or_download_build(&dir, &wrong, |_, _| {}, Some(&cancel)).unwrap_err();
    assert!(error.is_cancelled());

    assert_eq!(names(&dir), before);
    assert_eq!(fs::read_to_string(dir.join(RECORD)).unwrap(), record);
    assert_eq!(
        fs::read_to_string(dir.join(Tool::Ffmpeg.file_name())).unwrap(),
        "old"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_pair_that_does_not_run_is_an_error_naming_it() {
    let dir = scratch("broken");
    for tool in [Tool::Ffmpeg, Tool::Ffprobe] {
        fs::write(dir.join(tool.file_name()), "not a program").unwrap();
    }
    // Put there by hand (no record): kept, and checked.
    let build = FfmpegBuild::new("https://example.org/unused.zip", "0".repeat(64));
    let error = Tools::find_or_download_build(&dir, &build, |_, _| {}, None).unwrap_err();
    assert!(
        matches!(error, Error::Run { .. } | Error::Failed { .. }),
        "{error:?}"
    );
    assert!(
        error.to_string().contains(&Tool::Ffmpeg.file_name()),
        "{error}"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_pinned_build_is_used_where_there_is_one() {
    let dir = scratch("pinned");
    let cancel = AtomicBool::new(true);
    let result = Tools::find_or_download(&dir, |_, _| {}, Some(&cancel));
    match FfmpegBuild::pinned() {
        // Cancelled before anything is asked of the network.
        Some(_) => assert!(result.unwrap_err().is_cancelled()),
        None => assert!(matches!(result.unwrap_err(), Error::InvalidInput(_))),
    }
    fs::remove_dir_all(dir).unwrap();
}
