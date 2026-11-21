use super::*;
use std::path::Path;

#[test]
fn windows_and_linux_have_pinned_builds_and_macos_has_none() {
    let windows = FfmpegBuild::pinned_for("windows", "x86_64").unwrap();
    assert!(
        windows.url.contains("9.0.2-essentials")
            && Path::new(&windows.url).extension() == Some("zip".as_ref())
    );
    // The Windows zip comes from its copy on Hugging Face first, then from gyan.dev.
    assert!(
        windows
            .url
            .starts_with("https://huggingface.co/datasets/Freonius/jerome-sources/resolve/")
    );
    assert_eq!(
        windows.mirrors,
        ["https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip"]
    );
    for arch in ["x86_64", "aarch64"] {
        let linux = FfmpegBuild::pinned_for("linux", arch).unwrap();
        assert!(linux.url.contains("7.0.2") && linux.url.ends_with("-static.tar.xz"));
    }
    // Windows on arm64 takes the x64 build, which it runs through its emulation.
    assert_eq!(FfmpegBuild::pinned_for("windows", "aarch64"), Some(windows));
    assert_eq!(FfmpegBuild::pinned_for("macos", "aarch64"), None);
    assert_eq!(
        FfmpegBuild::pinned(),
        FfmpegBuild::pinned_for(std::env::consts::OS, std::env::consts::ARCH)
    );
}

#[test]
fn every_pin_is_a_lowercase_sha_256() {
    for (os, arch) in [
        ("windows", "x86_64"),
        ("linux", "x86_64"),
        ("linux", "aarch64"),
    ] {
        let build = FfmpegBuild::pinned_for(os, arch).unwrap();
        assert_eq!(build.sha256.len(), 64, "{build:?}");
        assert!(
            build
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "{build:?}"
        );
        assert!(build.url.starts_with("https://"), "{build:?}");
    }
}

#[test]
fn a_build_of_ones_own_is_kept_as_given() {
    let build = FfmpegBuild::new("https://example.org/ff.zip", "ab".repeat(32));
    assert_eq!(build.url, "https://example.org/ff.zip");
    assert_eq!(build.sha256, "ab".repeat(32));
    assert!(build.mirrors.is_empty());
    let mirrored = build.clone().with_mirrors(["https://example.net/ff.zip"]);
    assert_eq!(
        mirrored.sources().collect::<Vec<_>>(),
        ["https://example.org/ff.zip", "https://example.net/ff.zip"]
    );
    assert!(mirrored.same_archive(&build));
    assert!(!build.same_archive(&FfmpegBuild::new(&build.url, "cd".repeat(32))));
}

#[test]
fn an_entry_with_one_url_reads_and_writes_as_before() {
    let old = r#"{"url":"https://example.org/ff.zip","sha256":"ab"}"#;
    let build: FfmpegBuild = serde_json::from_str(old).unwrap();
    assert_eq!(build, FfmpegBuild::new("https://example.org/ff.zip", "ab"));
    assert_eq!(serde_json::to_string(&build).unwrap(), old);
}

#[test]
fn an_entry_with_mirrors_lists_them_after_its_url() {
    let build: FfmpegBuild = serde_json::from_str(
        r#"{"url":"https://a.example/ff.zip","mirrors":["https://b.example/ff.zip","https://c.example/ff.zip"],"sha256":"ab"}"#,
    )
    .unwrap();
    assert_eq!(
        build.sources().collect::<Vec<_>>(),
        [
            "https://a.example/ff.zip",
            "https://b.example/ff.zip",
            "https://c.example/ff.zip"
        ]
    );
    let again: FfmpegBuild = serde_json::from_str(&serde_json::to_string(&build).unwrap()).unwrap();
    assert_eq!(again, build);
}

#[test]
fn every_mirror_in_the_manifest_is_https() {
    for (os, arch) in [
        ("windows", "x86_64"),
        ("linux", "x86_64"),
        ("linux", "aarch64"),
    ] {
        let build = FfmpegBuild::pinned_for(os, arch).unwrap();
        assert!(
            build.sources().all(|url| url.starts_with("https://")),
            "{build:?}"
        );
    }
}
