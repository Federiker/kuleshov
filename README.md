# kuleshov

A Rust library that drives FFmpeg and FFprobe for audio and video, dedicated to
the public domain. It is named after Lev Kuleshov (1899–1970), the theorist of
film editing, who showed that a cut makes meaning out of what it joins.

kuleshov was split out of [manuzio](https://github.com/Federiker/manuzio), whose
audiobooks and trailers it encodes, and it is shared with
[melies](https://github.com/Federiker/melies). Its output is the same as
manuzio's own crates produced, argument for argument and byte for byte; a
parity test checks it ([docs/parity.md](docs/parity.md)).

- **Audio:** durations; concatenation into one mono MP3 (variable or constant
  bit rate, room tone instead of digital silence, ID3 tags); sample-exact WAV
  slots and their concatenation; `analyze_audio`, which measures the level,
  peak, noise floor, lead-in and tail of a file.
- **Video:** motion from a still image (a centre crop to the frame, zooms and
  overscan pans with the camera computed for every frame on a supersampled
  image, fades); joining clips, by stream copy only when their H.264
  parameters match, filtered otherwise; muxing audio; durations.
- **Processes:** every run can be cancelled, which stops the tool and removes
  partial outputs and temporary concat lists; paths reach FFmpeg as the system
  gives them (names that are not valid Unicode, folders with Chinese characters
  and apostrophes); errors name the file; on Windows no console window flashes.

## FFmpeg is yours to supply

kuleshov **never bundles FFmpeg, and never downloads it unless asked**. The
program that uses it says where `ffmpeg` and `ffprobe` are, and ships or
installs them itself under FFmpeg's own licence (LGPL or GPL, depending on the
build). Without instructions, kuleshov runs the bare names, which the system
finds on `PATH`.

With the `download` feature, the program may instead ask kuleshov to install
the pinned build for its platform into a folder of its choosing, once:
`Tools::find_or_download(dir, progress, cancel)` uses `ffmpeg` and `ffprobe`
from `dir` when they are there, and otherwise downloads the build, checks its
SHA-256, takes only the two programs out of the archive and checks that they
run. Nothing else in kuleshov touches the network. The pinned builds are in
[ffmpeg-builds.json](crates/kuleshov/ffmpeg-builds.json): gyan.dev's 9.0.2
essentials build on Windows x64, johnvansickle's 7.0.2 static builds on Linux
x64 and arm64, the builds the parity references belong to; none on macOS.
[`examples/install_ffmpeg.rs`](crates/kuleshov/examples/install_ffmpeg.rs) shows
the call. `Tools::find_or_download_build` takes a build of the program's choosing
instead. Both pinned builds are GPL-3.0: whether a program offers them to its
users, and how, is that program's decision.

```rust
use kuleshov::{CombineOptions, Mp3Encoding, Tool, Tools, combine_audio_files_with_options};

// Explicit paths, folders searched once, or a resolver asked every time.
let tools = Tools::find_in(["/opt/myapp/bin"]);
let tools = Tools::new("/usr/bin/ffmpeg", "/usr/bin/ffprobe");
let tools = Tools::with_resolver(|tool: Tool| my_settings().path_of(tool.name()));
let tools = Tools::default(); // ffmpeg and ffprobe from PATH
// With the `download` feature: installed into the folder once, if missing.
let tools = Tools::find_or_download(&app_data.join("ffmpeg"), |done, total| show(done, total), None)?;

let options = CombineOptions {
    encoding: Mp3Encoding::Cbr(192),
    ..CombineOptions::default()
};
let mp3 = combine_audio_files_with_options(&tools, &parts, 44_100, &out_dir, &options, None)?;
```

Every function that runs a tool takes `&Tools` first; the `_with_cancel`
variants take an `Option<&AtomicBool>` last and stop when it turns true.
[`src/lib.rs`](crates/kuleshov/src/lib.rs) lists the whole API.

### Errors

Every fallible function returns [`kuleshov::Error`](crates/kuleshov/src/error.rs):
`Cancelled`, `InvalidInput`, `UnsupportedPath`, `Run` (the program could not
be started), `Failed` (it ran and failed: its exit code, the last lines of its
standard error and the file it was working on), `Probe` (its answer about a
file could not be used), `Io` and `Download` (the server, the checksum or the
archive of an FFmpeg build failed). `error.is_cancelled()` and `error.file()`
answer the common questions; the library never uses `anyhow`.

### Features

- `tokio` (off by default): `audio_duration_async` and
  `combine_audio_files_async`, run on Tokio's blocking pool.
- `download` (off by default): `Tools::find_or_download`,
  `Tools::find_or_download_build` and `FfmpegBuild`, with blocking HTTPS
  (ureq and rustls), ZIP and tar.xz.

### Using it

kuleshov is not on crates.io. Pin the repository by commit:

```toml
kuleshov = { git = "https://github.com/Federiker/kuleshov", rev = "<commit>" }
```

It is tested on Windows and Linux, each with one pinned FFmpeg build
([docs/parity.md](docs/parity.md#the-reference-builds)); other builds and
platforms work but are not checked byte for byte.

## Licence

kuleshov is dedicated to the public domain under [CC0 1.0](LICENSE). FFmpeg is
not part of it and keeps its own licence.

## Setup

On a new machine, `scripts/install.sh` (macOS, Linux, Git Bash) or
`scripts\install.bat` (Windows) installs mise and the pinned tools
([mise.toml](mise.toml)), the Rust toolchain
([rust-toolchain.toml](rust-toolchain.toml)), Bun's packages and the Git hooks.
Run it again at any time.

The tests run FFmpeg. `mise run tools` downloads the pinned build for this
platform once (its SHA-256 is checked; the pins are those of
[ffmpeg-builds.json](crates/kuleshov/ffmpeg-builds.json), which
[scripts/downloads.ts](scripts/downloads.ts) reads)
and stages `ffmpeg` and `ffprobe` in the gitignored `tools/`; the tests find
them through `KULESHOV_TOOLS_DIR`, which [mise.toml](mise.toml) points there.
To use FFmpeg from elsewhere, such as a sibling manuzio checkout's staged
copy, set it in a gitignored `mise.local.toml`:

```toml
[env]
KULESHOV_TOOLS_DIR = "../manuzio/assets/tools"
```

The parity test needs the reference build for the platform; the other tests
run with any recent FFmpeg.

`mise tasks` lists the commands; the main ones:

| Command | Does |
| --- | --- |
| `mise run tools` | Stage the pinned FFmpeg in `tools/` |
| `mise run check` | `cargo check` of the workspace, and with the `tokio` feature |
| `mise run test` | The tests (nextest), the parity test among them |
| `mise run coverage` | The tests, failing under 90% lines or regions |
| `mise run clippy` | Clippy, warnings denied |
| `mise run fix` | Format the Rust code and the scripts |
| `mise run audit` | Dependency advisories, licences and sources |
| `mise run ci` | Everything CI runs |
| `mise run tag` | Sets the version, commits it, tags `v<version>` and pushes |

## Releasing

A release is the tag `v<version>` on `main`, with the version set once in the
root `Cargo.toml` (`[workspace.package]`). There is no GitHub release or
crates.io upload (the crates are `publish = false`), so plain Git is enough:
no token, only the credentials you already push with. Programs still pin the
commit a tag names (`git rev-parse v<version>^{commit}`), which no later
change to the tag can move.

Run `mise run tag` on `main` with everything committed and pushed or
pulled. It asks the new version (Enter for the next patch, or the current
one when it was never tagged), writes it to `Cargo.toml` and `Cargo.lock`
and commits those alone (`[release] kuleshov <version>` by default, with
the hooks), tags `v<version>` (annotated), then asks before
`git push --atomic origin main v<version>`. Nothing changes before the
commit; if you stop after it, it says what it did, how to push later and
how to undo it here. The tool is the `kuleshov_tag` crate, copied from
jespersen's.

## Local scratch data

The gitignored `tmp/` folder holds downloads, the harness that records the
parity references from manuzio, experiments and disposable scripts. Nothing in
it is a build input.

## Documentation

[docs/](docs/README.md): the decisions, the parity references and how they
were recorded, and the manuzio commit the code was copied from.

## Git conventions

[.gitattributes](.gitattributes) normalises text to LF, with CRLF for Windows
batch/command scripts.

### Hooks

[Lefthook](lefthook.yml) runs three hooks, installed by the install scripts:

- **before a commit**, rustfmt formats the staged Rust files and Biome the
  staged scripts and JSON, and the results are staged;
- **on the commit message**, the first line must start with a prefix below;
- **before a push**, Clippy (`mise run clippy`) and the scripts' type check
  (`mise run typecheck`).

`git commit --no-verify` skips them; CI runs the same checks anyway.

### Commit messages

The first line is `[prefix] What changed`, in the imperative and UK English,
for example `[fix] A cancelled concat removes its list`. One commit, one
prefix: when a change is two kinds of work, it is usually two commits. Git's
own `Merge`, `Revert` and `fixup!` messages are accepted as they are.

| Prefix | For | Why it is its own word |
| --- | --- | --- |
| `[add]` | Something new: a function, an option | The most common kind of change; its tests and docs come with it |
| `[fix]` | A bug fixed | Lets a reader find when a behaviour was wrong and when it was mended |
| `[remove]` | A feature, file or dead code taken out | Removal is a decision worth finding again |
| `[refactor]` | Code reorganised with the same behaviour | Tells a reviewer that no output should change (the parity test agrees) |
| `[rename]` | Files, crates or identifiers renamed or moved, nothing else | Moves make large diffs; keeping them alone keeps history readable |
| `[perf]` | Faster or smaller, same behaviour | Its measurements belong in the message |
| `[data]` | References re-recorded or fixtures changed | Their source (the manuzio commit, the FFmpeg build) belongs in the message |
| `[docs]` | Documentation only | Never changes what kuleshov does |
| `[test]` | Tests only | Adds confidence without touching the library |
| `[fmt]` | Formatting only | Large ones are listed in `.git-blame-ignore-revs` |
| `[build]` | Tooling, scripts, CI and configuration | The workshop, not the product |
| `[deps]` | Dependency updates (Renovate uses it) | Upgrades are where regressions often come from |
| `[security]` | A security fix or hardening | Easy to find when auditing |
| `[release]` | A version and its notes | Marks what shipped |
| `[revert]` | A change undone by hand | Says what was undone and why |
| `[init]` | A repository's first commit | Happens once |
