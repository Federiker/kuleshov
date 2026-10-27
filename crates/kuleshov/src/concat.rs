use crate::{Error, Result};
use std::path::Path;

/// One line of the list FFmpeg's concat demuxer reads: `file '<written>'`, for `file`.
///
/// A single quote inside a path ends the quoted name early, so it is written as `'\''` (close the
/// quote, an escaped quote, reopen). Backslashes of Windows paths are literal inside the quotes.
/// The list is text, one path a line: a path that is not valid Unicode, or that holds a line
/// break, cannot be written in it and is refused, naming `file`.
pub(crate) fn concat_entry(written: &Path, file: &Path) -> Result<String> {
    let name = written.to_str().ok_or_else(|| Error::UnsupportedPath {
        path: file.to_owned(),
        reason: "a path that is not valid Unicode cannot go in a concat list",
    })?;
    if name.contains(['\n', '\r']) {
        return Err(Error::UnsupportedPath {
            path: file.to_owned(),
            reason: "a path with a line break cannot go in a concat list",
        });
    }
    Ok(format!("file '{}'", name.replace('\'', "'\\''")))
}

#[cfg(test)]
#[path = "../tests/unit/concat.rs"]
mod tests;
