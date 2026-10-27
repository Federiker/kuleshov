# TODO

The shared roadmap (see [AGENTS.md](AGENTS.md#shared-roadmap)). kuleshov takes
over manuzio's `mnz-audios`, `mnz-videos` and their part of `mnz-tools`, with
the same output ([docs/parity.md](docs/parity.md)).

## Foundation

- [ ] Repository (the owner creates `Federiker/kuleshov` and pushes by hand):
  - [ ] Checks green on Linux (a push) and Windows (a manual run), once it is
    on GitHub. Locally `mise run ci` passes on Windows, and the tests and the
    parity test pass on Linux (WSL, the 7.0.2 build, cross-compiled binaries).
- [ ] FFmpeg downloaded on request (owner, 27 October 2026): the `download`
  feature, `Tools::find_or_download`, the pins in one manifest shared with
  `scripts/downloads.ts`.

## manuzio and melies

- [ ] manuzio switches to kuleshov (manuzio's `SPLITTER-HERE.md`, slice 1;
  what changes there: [docs/manuzio-source.md](docs/manuzio-source.md#switching-manuzio)):
  - [ ] Port what changed in manuzio's crates since the copied commit.
  - [ ] The parity test passes against manuzio's staged FFmpeg at the pinned
    commit.
- [ ] melies uses kuleshov for its encodes and trailers (melies' own roadmap);
  its music mix runs FFmpeg itself through `Tools::command` and
  `run_cancellable`.

## Later, if wanted

- [ ] macOS: no pinned FFmpeg build or references (manuzio has none either);
  CI covers Windows and Linux only.
