use super::*;
use std::path::PathBuf;

#[test]
fn one_line_per_file_in_order() {
    let list = concat_list(&[PathBuf::from("a.wav"), PathBuf::from("b.wav")]).unwrap();
    assert_eq!(list, "file 'a.wav'\nfile 'b.wav'");
}

#[test]
fn an_apostrophe_in_a_path_cannot_end_the_quoted_name() {
    let list = concat_list(&[PathBuf::from(r"C:\Users\O'Brien\L'Étranger\audio\1.wav")]).unwrap();
    assert_eq!(
        list,
        r"file 'C:\Users\O'\''Brien\L'\''Étranger\audio\1.wav'"
    );
    // Chinese and spaces need no escaping.
    assert_eq!(
        concat_list(&[PathBuf::from("张伟的文档/我的 书/一.wav")]).unwrap(),
        "file '张伟的文档/我的 书/一.wav'"
    );
}

#[test]
fn a_path_the_list_cannot_hold_is_refused_before_anything_is_written() {
    let dir = std::env::temp_dir().join(format!("kuleshov-list-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let tools = Tools::new("kuleshov-no-ffmpeg", "kuleshov-no-ffprobe");
    let bad = PathBuf::from("first\nsecond.wav");
    let error = combine_audio_files(&tools, std::slice::from_ref(&bad), 44_100, &dir).unwrap_err();
    assert_eq!(error.file(), Some(bad.as_path()), "{error}");
    let error = crate::concat_wav(&tools, std::slice::from_ref(&bad), 24_000, &dir).unwrap_err();
    assert_eq!(error.file(), Some(bad.as_path()), "{error}");
    assert!(!dir.exists());
}
