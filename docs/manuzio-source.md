# Copied from manuzio

Kuleshov is split out of [manuzio](https://github.com/Federiker/manuzio). Its code and tests were copied
from this commit, whose crates `crates/audios`, `crates/videos` and `crates/tools` were unchanged in the
working tree:

| | |
| --- | --- |
| Commit | `0ec8e8ede4be114c3f7d715f196a622e94c1b989` |
| Subject | [deps] Slice 12: kuchiki and html5ever 0.25 replaced by dom_query 0.29 (editing) and scraper 0.29 (reading) |
| Copied on | 27 October 2026 |

| manuzio | Here |
| --- | --- |
| `mnz-audios` (`crates/audios`) | duration, concatenation, slot placement, `analyze_audio` |
| `mnz-videos` (`crates/videos`) | motion from a still image, concatenation, audio muxing, duration |
| `mnz-tools` (`crates/tools`): `command`, `hide_console`, `run_cancellable`, `ProcessError`, `stderr_tail` | finding and running the tools |

Before manuzio switches to kuleshov, port whatever changed there since this commit:

```sh
git -C ../manuzio log 0ec8e8ede4be114c3f7d715f196a622e94c1b989..HEAD -- crates/audios crates/videos crates/tools
```

## Switching manuzio

What changes in manuzio when it uses kuleshov (its `SPLITTER-HERE.md`, slice 1):

| manuzio now | With kuleshov |
| --- | --- |
| `mnz_audios::f(args…)`, `mnz_videos::f(args…)` | `kuleshov::f(&tools, args…)`: the same names and arguments |
| `mnz_tools::command("ffmpeg")` | `tools.command(kuleshov::Tool::Ffmpeg)`, also without a console window |
| `mnz_tools::run_cancellable` after an FFmpeg command (the trailer's music mix) | `kuleshov::run_cancellable`, which returns `kuleshov::Error` |
| `error.downcast_ref::<mnz_tools::ProcessError>()` matching `Cancelled` | `error.downcast_ref::<kuleshov::Error>().is_some_and(kuleshov::Error::is_cancelled)` |
| `anyhow::Result` from the media crates | `kuleshov::Result`; `?` turns it into `anyhow` at manuzio's boundaries |

The tools, found as manuzio finds them now (`MNZ_TOOLS_DIR`, the resource
folder, next to the executable, then `PATH`, read at every call), are one
adapter in `mnz-tools`:

```rust
pub fn media_tools() -> kuleshov::Tools {
    kuleshov::Tools::with_resolver(|tool| find_in(tool.name(), &tool_dirs()))
}
```

The messages manuzio's tests look for are kept: "ffprobe could not read" with
the file, "no input files provided" (audio), "No input files provided" (video),
"slot length must be positive", "mux duration must be finite and positive" and
"video dimensions must be positive and even".
