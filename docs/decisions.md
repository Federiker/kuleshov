# Decisions

Owner decisions, newest last. Each says what was decided and why, so that it
is not argued again without a reason.

## Foundation (27 October 2026)

1. **Purpose.** kuleshov takes over manuzio's `mnz-audios`, `mnz-videos` and
   the part of `mnz-tools` they use, so that manuzio and melies share one
   FFmpeg layer. Its output stays manuzio's ([parity.md](parity.md)).
2. **Copied, not rewritten.** The code and the tests are manuzio's, adapted
   only where they could not be generic: no manuzio names, no `MNZ_`
   variables, the tools passed in, typed errors.
3. **Licence.** CC0 1.0, like jespersen; open source. Not on crates.io:
   manuzio and melies pin the repository by commit.
4. **FFmpeg is the caller's.** Never bundled, downloaded at run time or looked
   for: the caller says where `ffmpeg` and `ffprobe` are, and `PATH` is the
   fallback. The library reads no environment variable.
5. **The API.** manuzio's functions keep their names and variants (plain,
   `_with_cancel`, `_with_options`, `_async`) and take `&Tools` first, so
   manuzio's call sites change by one argument. `Tools` holds explicit paths,
   finds each tool in folders once (`find_in`), or asks a resolver every time
   a command is built (`with_resolver`), which keeps manuzio's lookup, read at
   each call, as it was.
6. **One error type.** `kuleshov::Error`, written by hand (no `thiserror`),
   replaces `anyhow` and `ProcessError`: `Cancelled`, `InvalidInput`,
   `UnsupportedPath`, `Run`, `Failed` (program, file, exit code, standard
   error's tail), `Probe` and `Io`. `run_cancellable` returns it too, so a
   caller matches `Error::Cancelled` instead of downcasting. `Failed` names
   the file the tool was working on: the probed file, the input of a slot or a
   motion, the video of a mux, the output of a join or an encode of several
   inputs.
7. **The async variants** stay, behind the `tokio` feature (off by default):
   manuzio does not call them, and the library needs no runtime without them.
8. **The reference harness** lives in the gitignored `tmp/`, not in the
   workspace: it needs manuzio next to the repository. The parity test
   (committed) holds the same scenarios.
9. **Linux references** were recorded in WSL with the pinned 7.0.2 static
   build, so each platform has references for its own build.
10. **Every test runs by default**, those manuzio kept in its `engine` tier
    included: FFmpeg is what this library is about, and CI stages it anyway.
11. **CI** checks Linux on pushes and pull requests, and Windows (or both) on
    manual runs. No macOS: there is no pinned FFmpeg build for it, in manuzio
    either.
12. **FFmpeg for the tests** comes from `KULESHOV_TOOLS_DIR`, a test-only
    variable: `tools/` staged by `mise run tools` by default, or a folder set
    in the gitignored `mise.local.toml`, such as manuzio's staged copy.

## Made while porting (27 October 2026)

These are the session's, within the decisions above; ask the owner before
undoing one.

13. **Formatting** keeps rustfmt's default width (100), as manuzio does, so
    that copied code and later ports compare line by line with manuzio's.
14. **`unsafe` is denied, not forbidden:** the one Windows test that asks
    whether a child has a console window calls `GetConsoleWindow`, under
    `#[expect(unsafe_code)]`, as manuzio's does.
15. **Small changes from manuzio**, none of which changes an argument or an
    output (the parity test agrees):
    - `combine_video_files` passes its output path as the system gives it,
      instead of through `to_string_lossy`;
    - a concat list that cannot be written is removed;
    - the ffprobe failure of `concat_matching_video_files_with_cancel` names
      the clip, and every probe failure keeps the tail of ffprobe's standard
      error;
    - the `Cancelled` error reads "cancelled" (manuzio's "Process cancelled"
      came wrapped in the operation's context).
