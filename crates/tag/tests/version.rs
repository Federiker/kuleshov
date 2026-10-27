//! Versions and their order.

use kuleshov_tag::Version;

#[expect(
    clippy::unwrap_used,
    reason = "a test helper; a failure should fail the test"
)]
fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

#[test]
fn versions_are_x_y_z_with_an_optional_prerelease() {
    assert_eq!(version("0.1.0").to_string(), "0.1.0");
    assert_eq!(version(" 1.2.3 ").to_string(), "1.2.3");
    assert_eq!(version("0.2.0-alpha.1").prerelease, ["alpha", "1"]);
    assert_eq!(version("1.0.0-rc-1.x7").to_string(), "1.0.0-rc-1.x7");
    for wrong in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "01.2.3",
        "1.02.3",
        "v1.2.3",
        "1.2.x",
        "1.2.3-",
        "1.2.3-a..b",
        "1.2.3-01",
        "1.2.3-a_b",
        "1.2.-3",
    ] {
        assert_eq!(Version::parse(wrong), None, "{wrong:?}");
    }
}

#[test]
fn versions_follow_semver_precedence() {
    let order = [
        "0.1.0",
        "0.1.1",
        "0.2.0-alpha",
        "0.2.0-alpha.1",
        "0.2.0-alpha.2",
        "0.2.0-alpha.10",
        "0.2.0-alpha.beta",
        "0.2.0-beta",
        "0.2.0",
        "1.0.0",
    ];
    for pair in order.windows(2) {
        assert!(
            version(pair[0]) < version(pair[1]),
            "{} < {}",
            pair[0],
            pair[1]
        );
    }
    assert_eq!(
        version("1.0.0").cmp(&version("1.0.0")),
        std::cmp::Ordering::Equal
    );
}

#[test]
fn the_next_version_is_the_next_patch_or_the_release() {
    assert_eq!(version("0.1.0").next_patch().to_string(), "0.1.1");
    assert_eq!(version("0.2.0-alpha.1").next_patch().to_string(), "0.2.0");
    assert_eq!(version("0.1.0").tag(), "v0.1.0");
}
