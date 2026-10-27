use super::*;
use std::path::Path;

#[test]
fn windows_and_linux_have_pinned_builds_and_macos_has_none() {
    let windows = FfmpegBuild::pinned_for("windows", "x86_64").unwrap();
    assert!(
        windows.url.contains("9.0.2-essentials")
            && Path::new(&windows.url).extension() == Some("zip".as_ref())
    );
    for arch in ["x86_64", "aarch64"] {
        let linux = FfmpegBuild::pinned_for("linux", arch).unwrap();
        assert!(linux.url.contains("7.0.2") && linux.url.ends_with("-static.tar.xz"));
    }
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
}
