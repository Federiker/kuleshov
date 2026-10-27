//! The root Cargo.toml's `[workspace.package]` version, read and rewritten, and the Cargo.lock a
//! version change should give. Pure functions on text, which keep its line endings.

use std::{fmt::Write, ops::Range};

/// Where the value of the `version = "…"` line of `[workspace.package]` is in `cargo_toml`.
fn version_value(cargo_toml: &str) -> Option<Range<usize>> {
    let mut in_section = false;
    let mut offset = 0;
    for line in cargo_toml.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == "[workspace.package]";
            continue;
        }
        if !in_section {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("version") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        let length = value.find('"')?;
        let from = start + line.find(value)?;
        return Some(from..from + length);
    }
    None
}

/// The root Cargo.toml's `[workspace.package]` version, or `None` when it has none.
#[must_use]
pub fn workspace_version(cargo_toml: &str) -> Option<&str> {
    version_value(cargo_toml).map(|range| &cargo_toml[range])
}

/// The root Cargo.toml with `version` as its `[workspace.package]` version, or `None` when it
/// has none.
#[must_use]
pub fn with_workspace_version(cargo_toml: &str, version: &str) -> Option<String> {
    let range = version_value(cargo_toml)?;
    Some(format!(
        "{}{version}{}",
        &cargo_toml[..range.start],
        &cargo_toml[range.end..]
    ))
}

/// The Cargo.lock a version change should give: the workspace's own crates (the packages without
/// a `source`, which the registry's and git's have) at `version`, nothing else changed.
#[must_use]
pub fn lock_with_version(cargo_lock: &str, version: &str) -> String {
    let mut blocks: Vec<Vec<&str>> = vec![Vec::new()];
    for line in cargo_lock.split_inclusive('\n') {
        if line.trim_end() == "[[package]]" {
            blocks.push(Vec::new());
        }
        if let Some(block) = blocks.last_mut() {
            block.push(line);
        }
    }
    let mut out = String::with_capacity(cargo_lock.len());
    for block in &blocks {
        let package = block
            .first()
            .is_some_and(|line| line.trim_end() == "[[package]]");
        let own = package && !block.iter().any(|line| line.starts_with("source = "));
        let mut replaced = false;
        for line in block {
            if own && !replaced && line.starts_with("version = \"") {
                let ending = &line[line.trim_end().len()..];
                // Writing to a String cannot fail.
                let _ = write!(out, "version = \"{version}\"{ending}");
                replaced = true;
            } else {
                out.push_str(line);
            }
        }
    }
    out
}
