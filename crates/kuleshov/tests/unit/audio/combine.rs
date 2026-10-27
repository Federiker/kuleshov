use super::concat_list;
use std::path::PathBuf;

#[test]
fn one_line_per_file_in_order() {
    let list = concat_list(&[PathBuf::from("a.wav"), PathBuf::from("b.wav")]);
    assert_eq!(list, "file 'a.wav'\nfile 'b.wav'");
}

#[test]
fn an_apostrophe_in_a_path_cannot_end_the_quoted_name() {
    let list = concat_list(&[PathBuf::from(r"C:\Users\O'Brien\L'Étranger\audio\1.wav")]);
    assert_eq!(
        list,
        r"file 'C:\Users\O'\''Brien\L'\''Étranger\audio\1.wav'"
    );
    // Chinese and spaces need no escaping.
    assert_eq!(
        concat_list(&[PathBuf::from("张伟的文档/我的 书/一.wav")]),
        "file '张伟的文档/我的 书/一.wav'"
    );
}
