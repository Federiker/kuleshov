//! What the download tests share: a local HTTP server, and archives built on the spot.

use std::{
    fmt::Write as _,
    io::{BufRead, BufReader, Cursor, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

/// A server on this machine that answers `GET /<name>` with the bytes of `name`, and 404 otherwise.
pub(crate) struct Server {
    base: String,
    requests: Arc<AtomicUsize>,
}

impl Server {
    pub(crate) fn new(files: Vec<(&'static str, Arc<Vec<u8>>)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = requests.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                counter.fetch_add(1, Ordering::Relaxed);
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                // The rest of the request, up to its blank line.
                let mut header = String::new();
                while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
                    header.clear();
                }
                let path = line
                    .split(' ')
                    .nth(1)
                    .unwrap_or("/")
                    .trim_start_matches('/');
                let _ = match files.iter().find(|(name, _)| *name == path) {
                    Some((_, body)) => write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .and_then(|()| stream.write_all(body)),
                    None => stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
            }
        });
        Self { base, requests }
    }

    pub(crate) fn url(&self, name: &str) -> String {
        format!("{}/{name}", self.base)
    }

    /// The requests answered so far.
    pub(crate) fn requests(&self) -> usize {
        self.requests.load(Ordering::Relaxed)
    }
}

/// Lowercase hexadecimal SHA-256 of `bytes`.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// A ZIP of `files` (`(path in the archive, contents)`), stored without compression.
pub(crate) fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (path, contents) in files {
        writer.start_file(*path, options).unwrap();
        writer.write_all(contents).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// A tar.xz of `files` (`(path in the archive, contents)`).
pub(crate) fn tar_xz(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    for (path, contents) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, path, *contents).unwrap();
    }
    let tar = tar.into_inner().unwrap();
    let mut xz =
        lzma_rust2::XzWriter::new(Vec::new(), lzma_rust2::XzOptions::with_preset(1)).unwrap();
    xz.write_all(&tar).unwrap();
    xz.finish().unwrap()
}

/// An empty folder for one test.
pub(crate) fn scratch(label: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("kuleshov-download-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
