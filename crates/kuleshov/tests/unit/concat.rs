use super::*;
use std::path::PathBuf;

#[test]
fn a_line_names_the_path_in_quotes() {
    let path = Path::new("张伟的文档/我的 书/一.wav");
    assert_eq!(
        concat_entry(path, path).unwrap(),
        "file '张伟的文档/我的 书/一.wav'"
    );
    let path = Path::new(r"C:\Users\O'Brien\1.wav");
    assert_eq!(
        concat_entry(path, path).unwrap(),
        r"file 'C:\Users\O'\''Brien\1.wav'"
    );
}

#[test]
fn a_line_break_is_refused_naming_the_file() {
    let written = PathBuf::from("/work/a\nb.wav");
    let error = concat_entry(&written, Path::new("a\nb.wav")).unwrap_err();
    assert!(matches!(error, Error::UnsupportedPath { .. }), "{error:?}");
    assert_eq!(error.file(), Some(Path::new("a\nb.wav")));
    assert!(error.to_string().contains("line break"), "{error}");
}

#[cfg(windows)]
#[test]
fn a_name_that_is_not_unicode_is_refused() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    let mut name: Vec<u16> = "tone-".encode_utf16().collect();
    name.push(0xD800);
    let path = PathBuf::from(OsString::from_wide(&name));
    let error = concat_entry(&path, &path).unwrap_err();
    assert!(error.to_string().contains("not valid Unicode"), "{error}");
}

#[cfg(unix)]
#[test]
fn a_name_that_is_not_unicode_is_refused() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let path = PathBuf::from(OsStr::from_bytes(b"chapitre-\xe9t\xe9.wav"));
    let error = concat_entry(&path, &path).unwrap_err();
    assert!(error.to_string().contains("not valid Unicode"), "{error}");
}
