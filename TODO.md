# TODO

The shared roadmap (see [AGENTS.md](AGENTS.md#shared-roadmap)). kuleshov takes
over manuzio's `mnz-audios`, `mnz-videos` and their part of `mnz-tools`, with
the same output ([docs/parity.md](docs/parity.md)).

## Foundation

- [ ] Repository (the owner creates `Federiker/kuleshov` and pushes by hand):
  - [ ] Checks green on Linux (a push) and Windows (a manual run), once it is
    on GitHub. Locally `mise run ci` passes on Windows, and the tests and the
    parity test pass on Linux (WSL, the 7.0.2 build, cross-compiled binaries).
- [ ] Coverage: the threshold is 80% of lines and regions, as in jespersen;
  measured 93.8% of lines and 93.0% of regions on Windows (27 October 2026).
  Raise it to 90% if the owner agrees.
- [ ] The full-HD motion test (`supersampled_motion_reduces_zoom_jitter_…`)
  takes 40 s on Windows and 64 s in WSL, most of the test time: keep it, or
  measure fewer frames? Ask the owner.

## manuzio and melies

- [ ] manuzio switches to kuleshov (manuzio's `SPLITTER-HERE.md`, slice 1;
  what changes there: [docs/manuzio-source.md](docs/manuzio-source.md#switching-manuzio)):
  - [ ] Port what changed in manuzio's crates since the copied commit.
  - [ ] The parity test passes against manuzio's staged FFmpeg at the pinned
    commit.
- [ ] melies uses kuleshov for its encodes and trailers (melies' own roadmap);
  its music mix runs FFmpeg itself through `Tools::command` and
  `run_cancellable`.

## Behaviour kept from manuzio, to decide

Each changes an argument or an output, so the parity references would be
recorded again (from manuzio, after the same change there) or the scenario
excluded: the owner decides.

- [ ] Room tone is random: `anoisesrc` gets no seed, so two retail encodes of
  the same speech differ. A fixed seed (`seed=…`) would make them
  reproducible and let the parity test compare their bytes.
- [ ] An audio concat list writes each path through `to_string_lossy`, so a
  name that is not valid Unicode is altered in the list; the video join
  refuses such a path with `Error::UnsupportedPath`. Refuse it in the audio
  lists too?
- [ ] `analyze_audio` cannot be cancelled while it decodes (a long chapter
  takes seconds); a `_with_cancel` variant would stop it.

## Later, if wanted

- [ ] macOS: no pinned FFmpeg build or references (manuzio has none either);
  CI covers Windows and Linux only.
