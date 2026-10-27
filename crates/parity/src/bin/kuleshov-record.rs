//! The recording shim ([`kuleshov_parity::Shim`]): copied as `ffmpeg` and `ffprobe` into a folder
//! that the code under test takes for its tools folder, it logs each call and runs the real tool.

use std::process::ExitCode;

fn main() -> ExitCode {
    kuleshov_parity::Shim {
        name: "kuleshov-record",
        wavs: false,
    }
    .run()
}
