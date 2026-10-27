# Project guidance

Read [README.md](README.md) for setup and the API, [docs/parity.md](docs/parity.md)
before changing anything that reaches FFmpeg, and
[docs/manuzio-source.md](docs/manuzio-source.md) for the manuzio commit the code
was copied from.

## The same output as manuzio (read first)

kuleshov drives FFmpeg and FFprobe for manuzio and melies, which pin it by
commit. It was split out of manuzio's `mnz-audios`, `mnz-videos` and part of
`mnz-tools`, and must produce exactly what they produced:

- The committed references in `crates/parity/references/` (the FFmpeg and
  FFprobe arguments of every public function over fixed inputs, and the
  outputs' hashes) were **recorded from manuzio's crates**, with the harness in
  the gitignored `tmp/`. They are only ever re-recorded from manuzio, the same
  way; never from kuleshov itself, and never edited by hand to make the parity
  test pass.
- The references belong to one FFmpeg build per platform (Windows: gyan.dev
  9.0.2 essentials; Linux: johnvansickle 7.0.2 amd64 static, both pinned in
  [scripts/downloads.ts](scripts/downloads.ts)). Changing a pin means new
  references: ask the owner first.
- A change that alters the arguments or the output is a decision for the
  owner, not a refactor: ask before making it.
- Paths reach FFmpeg as the system gives them (names that are not valid
  Unicode, folders with Chinese characters and apostrophes); errors name the
  file; cancellation stops the child and removes partial outputs and temporary
  concat lists; no console window flashes on Windows. Each has its tests: keep
  them.

## Shared roadmap

- Read the root [TODO.md](TODO.md) at the start of each task to understand the
  remaining work and avoid duplicating another session's progress.
- Update relevant checklist items before finishing: add newly discovered
  work, split large tasks, note blockers and check completed items only after
  implementation and verification. A stub does not complete a feature.
- Write each TODO item as a short list of slices; they may change as the work
  shows what it needs. Tick a slice when it is done and verified; once the
  whole item is done and described in the docs, remove it from the TODO
  (document it first if it is not).
- Keep roadmap changes focused on the current task; preserve other
  contributors' entries and unresolved decisions.

## Conventions

- Use UK English in identifiers, user-facing text and documentation. Names
  kept from manuzio's API for its callers (`analyze_audio`) and FFmpeg's own
  names (`zoompan`, `color_range`) stay as they are.
- Crates live in `crates/<name>`: `kuleshov` is the library other programs
  depend on; `kuleshov_parity` (never published) holds the recording shim and
  the parity test.
- Crates must inherit package metadata, dependencies and lints from the Cargo
  workspace wherever possible (`{ workspace = true }`); Clippy's `all` and
  `pedantic` warn, and CI denies warnings.
- No inline test modules: unit tests live in `crates/<crate>/tests/unit/…` and
  are included with `#[cfg(test)] #[path = "../tests/unit/<name>.rs"] mod tests;`;
  integration tests go in `crates/<crate>/tests/`. `lib.rs` and `mod.rs` stay
  declarative: modules and exports only.
- Typed errors: the library returns its own error type and never uses
  `anyhow`; applications decide how to report it. An error names the file it
  is about. Report errors explicitly; do not silently substitute empty or
  success-shaped values.
- The library reads no environment variables and finds nothing on its own:
  the caller says where ffmpeg and ffprobe are (explicit paths or a resolver,
  `PATH` as the fallback). Only the tests read `KULESHOV_TOOLS_DIR`.
- FFmpeg is never bundled, downloaded at run time or committed. Tests use the
  pinned build that `mise run tools` stages in the gitignored `tools/`, or the
  folder `KULESHOV_TOOLS_DIR` names (set it in the gitignored
  `mise.local.toml`). Never copy FFmpeg binaries into the repository; commit
  only small fixtures, never large media.
- Every test runs by default (`mise run test`); a test that needs FFmpeg fails
  clearly, naming the folder, when it is missing.
- Keep test coverage at 80% or more of lines and regions
  (`mise run coverage`). CI fails below; new code comes with its tests.
- Ask before decisions that materially change the public API, the FFmpeg
  arguments or output, or the licensing model (CC0 1.0).
- Not on crates.io: manuzio and melies pin the repository by commit. Until the
  first release, APIs need not stay backwards compatible: do not keep shims
  for older development versions.
- Prefer native Rust tooling and `mise` tasks; the only scripts are the
  TypeScript ones in `scripts/` (Bun), which stage FFmpeg.
- The version is set once, in the root `Cargo.toml` (`[workspace.package]`).
- Commit messages start with a `[prefix]`, as described in the README's "Git
  conventions" (the commit-msg hook checks it), for example
  `[fix] A cancelled concat removes its list`.

## Assistant conduct

- **Local notes:** if `tmp/docs/README.md` exists, read it at the start of
  each task and follow it; it holds local-only instructions that are
  deliberately not committed (`tmp/` is gitignored). Keep it up to date when
  you learn something that belongs there. If it does not exist, carry on
  without it.
- Use the gitignored root `tmp/` folder for local downloads, temporary
  scripts, the reference harness and experiments. Do not stage it or treat its
  contents as required build inputs. Keep durable findings in tracked docs.
- manuzio, jespersen and the other sibling repositories are read only from
  here. Do not run manuzio's app or its dev servers.
- Ask before downloading anything large (an FFmpeg build is one).
- Stop helper processes when finished. Retain reusable downloads when useful;
  remove only explicitly identified obsolete scratch files.
- Ask about material uncertainties instead of guessing: anything that
  affects the FFmpeg arguments or output, the public API or the licence.
- When clarification or approval is needed, prefer interactive questions with
  a small set of concrete choices and a recommended option. Use the available
  question tool when supported; otherwise offer numbered choices in chat.
  Allow a custom answer and ask only questions that materially affect the work.
  This preference applies to all agents following this shared guidance.

Be courteous and constructive. If a request includes insults or abusive
language directed at the assistant, politely decline to continue that request.
