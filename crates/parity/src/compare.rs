use crate::{Output, Reference, Scenario, Value};

/// How far a measured value of a room-tone scenario may move between two runs: the noise floor
/// of the same encode varied by about 0.6 dB between recordings.
pub const ROOM_TONE_TOLERANCE: f64 = 1.5;

/// Everything in `actual` that differs from `expected`, one line each; empty when they agree.
///
/// Arguments, concat lists and leftover files must be equal; results equal, or both errors; and
/// outputs equal byte for byte. The exception is FFmpeg's room tone (`anoisesrc`), whose noise is
/// seeded at random: its outputs agree in their stream parameters but bit rate and duration, and
/// values measured from them agree within [`ROOM_TONE_TOLERANCE`] (docs/parity.md).
#[must_use]
pub fn compare(expected: &Reference, actual: &Reference) -> Vec<String> {
    if (&expected.ffmpeg, &expected.ffprobe) != (&actual.ffmpeg, &actual.ffprobe) {
        return vec![format!(
            "the references belong to \"{}\" and \"{}\", not \"{}\" and \"{}\": point \
KULESHOV_TOOLS_DIR at that build (`mise run tools` stages it)",
            expected.ffmpeg, expected.ffprobe, actual.ffmpeg, actual.ffprobe
        )];
    }
    let names = |reference: &Reference| -> Vec<String> {
        reference.scenarios.iter().map(|s| s.name.clone()).collect()
    };
    if names(expected) != names(actual) {
        return vec![format!(
            "the scenarios differ: expected {:?}, got {:?}",
            names(expected),
            names(actual)
        )];
    }
    let random = random_scenarios(expected);
    let mut problems = Vec::new();
    for (want, got) in expected.scenarios.iter().zip(&actual.scenarios) {
        compare_scenario(want, got, random.contains(&want.name), &mut problems);
    }
    problems
}

/// The scenarios whose output is random: those that ask for room tone, and those that read
/// their outputs.
pub(crate) fn random_scenarios(reference: &Reference) -> Vec<String> {
    let mut random: Vec<String> = Vec::new();
    for scenario in &reference.scenarios {
        let text = serde_json::to_string(&scenario.calls).unwrap_or_default();
        if text.contains("anoisesrc")
            || random
                .iter()
                .any(|name| text.contains(&format!("{{{name}#")))
        {
            random.push(scenario.name.clone());
        }
    }
    random
}

fn compare_scenario(want: &Scenario, got: &Scenario, random: bool, problems: &mut Vec<String>) {
    let name = &want.name;
    if want.calls.len() != got.calls.len() {
        problems.push(format!(
            "{name}: {} tool calls, expected {}: {:?}",
            got.calls.len(),
            want.calls.len(),
            got.calls
        ));
    }
    for (index, (a, b)) in want.calls.iter().zip(&got.calls).enumerate() {
        if a != b {
            problems.push(format!(
                "{name}: call {index} differs\n  expected {a:?}\n       got {b:?}"
            ));
        }
    }
    if want.files != got.files {
        problems.push(format!(
            "{name}: left {:?}, expected {:?}",
            got.files, want.files
        ));
    }
    match (&want.result, &got.result) {
        (Value::Ok(a), Value::Ok(b)) if a == b || (random && close(a, b)) => {}
        (Value::Err(_), Value::Err(_)) => {}
        (a, b) => problems.push(format!("{name}: returned {b:?}, expected {a:?}")),
    }
    if want.outputs.len() != got.outputs.len() {
        problems.push(format!(
            "{name}: {} outputs, expected {}",
            got.outputs.len(),
            want.outputs.len()
        ));
    }
    for (a, b) in want.outputs.iter().zip(&got.outputs) {
        compare_output(name, a, b, random, problems);
    }
}

fn compare_output(
    name: &str,
    want: &Output,
    got: &Output,
    random: bool,
    problems: &mut Vec<String>,
) {
    if random {
        if masked(&want.streams) != masked(&got.streams) {
            problems.push(format!(
                "{name}: {} streams {:?}, expected {:?}",
                got.name, got.streams, want.streams
            ));
        }
    } else if want.sha256 != got.sha256 {
        let decoded = want.audio_md5 == got.audio_md5
            && want.video_framemd5 == got.video_framemd5
            && want.streams == got.streams;
        problems.push(format!(
            "{name}: {} has other bytes ({} instead of {}); decoded it is {}",
            got.name,
            got.bytes,
            want.bytes,
            if decoded { "the same" } else { "different" }
        ));
    }
}

/// Stream lines without the fields that follow the noise: bit rate and duration.
fn masked(streams: &[String]) -> Vec<String> {
    streams
        .iter()
        .map(|line| {
            line.split('|')
                .filter(|field| {
                    !["bit_rate=", "duration=", "duration_ts="]
                        .iter()
                        .any(|key| field.starts_with(key))
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect()
}

/// Whether two values written as text agree, their numbers within [`ROOM_TONE_TOLERANCE`].
pub(crate) fn close(a: &str, b: &str) -> bool {
    let (a, b) = (tokens(a), tokens(b));
    a.len() == b.len()
        && a.iter().zip(&b).all(|(x, y)| {
            x == y
                || matches!((x.parse::<f64>(), y.parse::<f64>()),
                    (Ok(x), Ok(y)) if (x - y).abs() <= ROOM_TONE_TOLERANCE)
        })
}

/// `text` cut into numbers (`-69.4`, `192000`) and the text between them.
pub(crate) fn tokens(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < bytes.len() {
        let number = bytes[i].is_ascii_digit()
            || (bytes[i] == b'-' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit));
        if number {
            if start < i {
                tokens.push(&text[start..i]);
            }
            let begin = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            tokens.push(&text[begin..i]);
            start = i;
        } else {
            i += 1;
        }
    }
    if start < text.len() {
        tokens.push(&text[start..]);
    }
    tokens
}

#[cfg(test)]
#[path = "../tests/unit/compare.rs"]
mod tests;
