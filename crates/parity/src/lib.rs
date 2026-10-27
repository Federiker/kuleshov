//! What keeps kuleshov's output the same as manuzio's: a recording shim that stands in for `ffmpeg`
//! and `ffprobe`, the fixtures every scenario reads, and the references recorded from manuzio's
//! crates (docs/parity.md).

mod compare;
mod fixtures;
mod outputs;
mod references;
mod session;
mod shim;

pub use compare::{ROOM_TONE_TOLERANCE, compare};
pub use fixtures::{Fixtures, ppm, wav};
pub use outputs::describe;
pub use references::{Call, Outcome, Output, Reference, Scenario, Value, platform};
pub use session::{CALLS_FILE, REAL_TOOLS_FILE, Session, executable, hidden};
pub use shim::Shim;
