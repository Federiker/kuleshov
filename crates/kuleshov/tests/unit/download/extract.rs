use super::*;
use crate::{
    Tool,
    download::support::{scratch, tar_xz, zip},
};
use std::fs;

/// An archive laid out as the pinned builds are: the programs in a folder, beside decoys.
fn files() -> Vec<(String, &'static [u8])> {
    vec![
        (
            "ffmpeg-9/doc/ffmpeg.html".to_owned(),
            b"a manual".as_slice(),
        ),
        (
            format!("ffmpeg-9/bin/{}", Tool::Ffprobe.file_name()),
            b"probe",
        ),
        (
            format!("ffmpeg-9/bin/{}", Tool::Ffmpeg.file_name()),
            b"mpeg",
        ),
        // A second copy further on is not taken.
        (format!("other/{}", Tool::Ffmpeg.file_name()), b"other"),
    ]
}

fn extract_from(kind: &str, archive: &[u8], dir: &Path) -> Result<()> {
    let path = dir.join(format!("download.{kind}"));
    fs::write(&path, archive).unwrap();
    let build = FfmpegBuild::new(format!("https://example.org/ff.{kind}"), "");
    let members = [
        (Tool::Ffmpeg.file_name(), dir.join("ffmpeg.part")),
        (Tool::Ffprobe.file_name(), dir.join("ffprobe.part")),
    ];
    let members: Vec<(String, &Path)> = members
        .iter()
        .map(|(name, path)| (name.clone(), path.as_path()))
        .collect();
    extract(&build, &path, &members, None)
}

#[test]
fn the_programs_are_taken_out_of_a_zip_and_a_tar_xz() {
    let owned = files();
    let files: Vec<(&str, &[u8])> = owned.iter().map(|(p, c)| (p.as_str(), *c)).collect();
    for (kind, archive) in [("zip", zip(&files)), ("tar.xz", tar_xz(&files))] {
        let dir = scratch(&format!("extract-{kind}"));
        extract_from(kind, &archive, &dir).unwrap();
        assert_eq!(
            fs::read(dir.join("ffmpeg.part")).unwrap(),
            b"mpeg",
            "{kind}"
        );
        assert_eq!(
            fs::read(dir.join("ffprobe.part")).unwrap(),
            b"probe",
            "{kind}"
        );
        // Nothing else was written.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 3, "{kind}");
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn a_missing_program_is_named() {
    let only = [(
        format!("bin/{}", Tool::Ffmpeg.file_name()),
        b"mpeg".as_slice(),
    )];
    let files: Vec<(&str, &[u8])> = only.iter().map(|(p, c)| (p.as_str(), *c)).collect();
    for (kind, archive) in [("zip", zip(&files)), ("tar.xz", tar_xz(&files))] {
        let dir = scratch(&format!("partial-{kind}"));
        let error = extract_from(kind, &archive, &dir).unwrap_err();
        assert!(matches!(error, Error::Download { .. }), "{error:?}");
        assert!(
            error.to_string().contains(&format!(
                "{} is not in the archive",
                Tool::Ffprobe.file_name()
            )),
            "{error}"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn an_archive_that_is_not_one_is_an_error() {
    for kind in ["zip", "tar.xz", "7z"] {
        let dir = scratch(&format!("broken-{kind}"));
        let error = extract_from(kind, b"not an archive", &dir).unwrap_err();
        assert!(matches!(error, Error::Download { .. }), "{error:?}");
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn a_cancelled_extraction_stops() {
    let owned = files();
    let files: Vec<(&str, &[u8])> = owned.iter().map(|(p, c)| (p.as_str(), *c)).collect();
    let dir = scratch("extract-cancel");
    let path = dir.join("download.zip");
    fs::write(&path, zip(&files)).unwrap();
    let build = FfmpegBuild::new("https://example.org/ff.zip", "");
    let target = dir.join("ffmpeg.part");
    let cancel = AtomicBool::new(true);
    let error = extract(
        &build,
        &path,
        &[(Tool::Ffmpeg.file_name(), target.as_path())],
        Some(&cancel),
    )
    .unwrap_err();
    assert!(error.is_cancelled());
    fs::remove_dir_all(dir).unwrap();
}
