use super::{FfmpegBuild, fetch::failed};
use crate::{Error, Result, process::cancelled};
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
};

/// Copy each member named by `members` (a file name, found at any depth of the archive) out of
/// `archive`, downloaded from `build`, into its target. Only regular files count: a folder or a
/// link of that name does not. Nothing outside the targets is written, whatever paths the archive
/// holds. The caller removes the targets on an error.
#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "the URL is lowercased first, and `.tar.xz` is two extensions"
)]
pub(crate) fn extract(
    build: &FfmpegBuild,
    archive: &Path,
    members: &[(String, &Path)],
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    let file = File::open(archive).map_err(Error::io(archive))?;
    let url = build.url.to_ascii_lowercase();
    let found = if url.ends_with(".zip") {
        from_zip(build, file, members, cancel)?
    } else if url.ends_with(".tar.xz") {
        from_tar_xz(build, file, members, cancel)?
    } else {
        return Err(failed(build, "it is neither a .zip nor a .tar.xz archive"));
    };
    match members.iter().zip(&found).find(|(_, found)| !**found) {
        Some(((name, _), _)) => Err(failed(build, format!("{name} is not in the archive"))),
        None => Ok(()),
    }
}

fn from_zip(
    build: &FfmpegBuild,
    file: File,
    members: &[(String, &Path)],
    cancel: Option<&AtomicBool>,
) -> Result<Vec<bool>> {
    let mut zip = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|error| failed(build, format!("its ZIP cannot be read: {error}")))?;
    let mut found = vec![false; members.len()];
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| failed(build, format!("its ZIP cannot be read: {error}")))?;
        if !entry.is_file() || entry.is_symlink() {
            continue;
        }
        let Some(name) = entry.enclosed_name().and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        }) else {
            continue;
        };
        if let Some(position) = wanted(members, &found, &name) {
            copy(build, &mut entry, members[position].1, cancel)?;
            found[position] = true;
        }
    }
    Ok(found)
}

fn from_tar_xz(
    build: &FfmpegBuild,
    file: File,
    members: &[(String, &Path)],
    cancel: Option<&AtomicBool>,
) -> Result<Vec<bool>> {
    let unreadable =
        |error: std::io::Error| failed(build, format!("its tar.xz cannot be read: {error}"));
    let xz = lzma_rust2::XzReader::new(BufReader::new(file), true);
    let mut tar = tar::Archive::new(xz);
    let mut found = vec![false; members.len()];
    for entry in tar.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(unreadable)?;
        let Some(name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if let Some(position) = wanted(members, &found, &name) {
            copy(build, &mut entry, members[position].1, cancel)?;
            found[position] = true;
            if found.iter().all(|found| *found) {
                break;
            }
        }
    }
    Ok(found)
}

/// The member called `name` that is still missing, if any.
fn wanted(members: &[(String, &Path)], found: &[bool], name: &str) -> Option<usize> {
    members
        .iter()
        .zip(found)
        .position(|((member, _), found)| !found && member == name)
}

fn copy(
    build: &FfmpegBuild,
    reader: &mut dyn Read,
    target: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    let mut out = File::create(target).map_err(Error::io(target))?;
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        if cancelled(cancel) {
            return Err(Error::Cancelled);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| failed(build, format!("its archive cannot be read: {error}")))?;
        if read == 0 {
            break;
        }
        out.write_all(&buffer[..read]).map_err(Error::io(target))?;
    }
    out.flush().map_err(Error::io(target))
}

#[cfg(test)]
#[path = "../../tests/unit/download/extract.rs"]
mod tests;
