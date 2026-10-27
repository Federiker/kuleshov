# TODO

The shared roadmap (see [AGENTS.md](AGENTS.md#shared-roadmap)). kuleshov takes
over manuzio's `mnz-audios`, `mnz-videos` and their part of `mnz-tools`, with
the same output ([docs/parity.md](docs/parity.md)).

## Foundation

- [ ] Repository (the owner creates `Federiker/kuleshov` and pushes by hand):
  - [ ] Checks green on Linux (a push) and Windows (a manual run), once it is
    on GitHub.
- [ ] Coverage: the threshold is 80% of lines and regions, as in jespersen;
  tune it once the tests have moved here and their real coverage is known.

## manuzio and melies

- [ ] manuzio switches to kuleshov (manuzio's `SPLITTER-HERE.md`, slice 1):
  - [ ] Port what changed in manuzio's crates since the copied commit
    ([docs/manuzio-source.md](docs/manuzio-source.md)).
  - [ ] The parity test passes against manuzio's staged FFmpeg at the pinned
    commit.
- [ ] melies uses kuleshov for its encodes and trailers (melies' own roadmap).

## Later, if wanted

- [ ] macOS: no pinned FFmpeg build or references (manuzio has none either);
  CI covers Windows and Linux only.
