//! Installs the FFmpeg build pinned for this platform into a folder, unless it is already there,
//! and prints where `ffmpeg` and `ffprobe` are:
//!
//! ```sh
//! cargo run -p kuleshov --features download --example install_ffmpeg -- <folder>
//! ```

use kuleshov::{Tool, Tools};
use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: install_ffmpeg <folder>");
        return ExitCode::from(2);
    };
    let mut shown = 0;
    let progress = |done: u64, total: Option<u64>| {
        // A line every 10 MB, and the last one.
        if done >= shown + 10_000_000 || total == Some(done) {
            shown = done;
            match total {
                Some(total) => eprintln!("{} of {} MB", done / 1_000_000, total / 1_000_000),
                None => eprintln!("{} MB", done / 1_000_000),
            }
        }
    };
    match Tools::find_or_download(&dir, progress, None) {
        Ok(tools) => {
            for tool in [Tool::Ffmpeg, Tool::Ffprobe] {
                println!("{tool}: {}", tools.path(tool).display());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
