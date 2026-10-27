use super::*;

/// A folder of its own for `name`, emptied first.
fn folder(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("kuleshov-shim-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn arg(path: &Path) -> OsString {
    path.as_os_str().to_owned()
}

#[test]
fn concat_lists_name_their_files() {
    assert_eq!(
        listed_files(
            "ffconcat version 1.0\nfile 'a.wav'\n  file 'O'\\''Brien.wav'  \nduration 1\n"
        ),
        ["a.wav", "O'Brien.wav"]
    );
    assert!(listed_files("file a.wav\nfile 'unclosed").is_empty());
}

#[test]
fn lists_are_recorded_and_wavs_only_when_asked() {
    let dir = folder("lists");
    let wav = dir.join("O'Brien.WAV");
    fs::write(&wav, b"RIFF").unwrap();
    let list = dir.join("list.txt");
    let text = format!(
        "file '{}'\nfile '{}'\n",
        wav.to_string_lossy().replace('\'', "'\\''"),
        dir.join("missing.wav").to_string_lossy()
    );
    fs::write(&list, &text).unwrap();
    let other = dir.join("other.wav");
    fs::write(&other, b"").unwrap();
    let args = [
        OsString::from("-i"),
        arg(&list),
        arg(&other),
        arg(&dir.join("absent.txt")),
        arg(&dir.join("out.mp3")),
    ];

    let plain = Shim {
        name: "test",
        wavs: false,
    }
    .lists(&args);
    assert_eq!(
        plain,
        BTreeMap::from([(list.to_string_lossy().into_owned(), text.clone())])
    );

    let with_wavs = Shim {
        name: "test",
        wavs: true,
    }
    .lists(&args);
    assert_eq!(
        with_wavs,
        BTreeMap::from([
            (list.to_string_lossy().into_owned(), text),
            (
                wav.to_string_lossy().into_owned(),
                "4 bytes, SHA-256 a40ff3d5900fb7698b8c865041347cb49eccedc8f93945f89629ad104aaecce4"
                    .to_owned()
            ),
            (
                other.to_string_lossy().into_owned(),
                "0 bytes, SHA-256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_owned()
            ),
        ])
    );
    let _ = fs::remove_dir_all(dir);
}
