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
