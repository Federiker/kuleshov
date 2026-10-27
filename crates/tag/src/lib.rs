//! `mise run tag`: sets kuleshov's version, commits it, tags it `v<version>` and pushes, asking
//! at each step. `main.rs` only passes the repository and the process's streams to [`run`], so
//! the tests can run it on a repository of their own.

mod manifest;
mod tag;
mod version;

pub use manifest::{lock_with_version, with_workspace_version, workspace_version};
pub use tag::run;
pub use version::Version;
