//! The version in the root Cargo.toml and Cargo.lock.

use kuleshov_tag::{lock_with_version, with_workspace_version, workspace_version};

const CARGO_TOML: &str = "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nedition = \"2024\"\nversion = \"0.1.0\"\n\n[workspace.dependencies]\nversion = \"9.9.9\"\n";

#[test]
fn the_workspace_version_is_read_from_its_section() {
    assert_eq!(workspace_version(CARGO_TOML), Some("0.1.0"));
    assert_eq!(workspace_version("[package]\nversion = \"1.0.0\"\n"), None);
    assert_eq!(
        workspace_version("[workspace.package]\nversion=\"2.0.0\"\r\n"),
        Some("2.0.0")
    );
    assert_eq!(
        workspace_version("[workspace.package]\nversion = \"2.0.0\n"),
        None,
        "unclosed"
    );
}

#[test]
fn the_workspace_version_is_rewritten_in_place() {
    let changed = with_workspace_version(CARGO_TOML, "0.2.0").unwrap();
    assert_eq!(
        changed,
        CARGO_TOML.replace("version = \"0.1.0\"", "version = \"0.2.0\"")
    );
    assert_eq!(with_workspace_version("[package]\n", "0.2.0"), None);
}

#[test]
fn only_the_workspace_crates_change_in_the_lock() {
    let lock = "version = 4\r\n\r\n[[package]]\r\nname = \"kuleshov\"\r\nversion = \"0.1.0\"\r\n\r\n[[package]]\r\nname = \"sha2\"\r\nversion = \"0.1.0\"\r\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\r\n";
    let expected = lock.replacen("version = \"0.1.0\"", "version = \"0.2.0\"", 1);
    assert_eq!(lock_with_version(lock, "0.2.0"), expected);
}
