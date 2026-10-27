use super::*;
use crate::Call;
use std::collections::BTreeMap;

fn call(args: &[&str]) -> Call {
    Call {
        tool: "ffmpeg".to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        lists: BTreeMap::new(),
    }
}

fn output(sha256: &str, streams: &[&str]) -> Output {
    Output {
        name: "{a#0}".to_owned(),
        bytes: 10,
        sha256: sha256.to_owned(),
        audio_md5: Some("MD5=1".to_owned()),
        video_framemd5: None,
        frames: None,
        streams: streams.iter().map(|line| (*line).to_owned()).collect(),
    }
}

fn scenario(name: &str, args: &[&str], result: Value, outputs: Vec<Output>) -> Scenario {
    Scenario {
        name: name.to_owned(),
        calls: vec![call(args)],
        result,
        outputs,
        files: Vec::new(),
    }
}

fn reference(scenarios: Vec<Scenario>) -> Reference {
    Reference {
        source: "test".to_owned(),
        ffmpeg: "ffmpeg version 1".to_owned(),
        ffprobe: "ffprobe version 1".to_owned(),
        scenarios,
    }
}

#[test]
fn numbers_are_cut_from_the_text_around_them() {
    assert_eq!(
        tokens("A { x: -69.4, y: Some(192000), z: \"a-b\" }"),
        vec![
            "A { x: ",
            "-69.4",
            ", y: Some(",
            "192000",
            "), z: \"a-b\" }"
        ]
    );
    assert_eq!(tokens("张伟 1.5"), vec!["张伟 ", "1.5"]);
    assert!(close(
        "floor: -69.40, rms: -26.6",
        "floor: -69.97, rms: -26.6"
    ));
    assert!(!close("floor: -69.40", "floor: -72.0"));
    assert!(!close("codec: \"mp3\"", "codec: \"aac\""));
}

#[test]
fn the_same_recording_agrees_and_another_build_is_named() {
    let expected = reference(vec![scenario(
        "a",
        &["-i", "x"],
        Value::Ok(String::new()),
        vec![output("aa", &["stream|bit_rate=1"])],
    )]);
    assert!(compare(&expected, &expected.clone()).is_empty());
    let mut other = expected.clone();
    other.ffmpeg = "ffmpeg version 2".to_owned();
    let problems = compare(&expected, &other);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("ffmpeg version 2"), "{problems:?}");
}

#[test]
fn arguments_files_results_and_bytes_must_match() {
    let expected = reference(vec![
        scenario(
            "a",
            &["-i", "x"],
            Value::Ok("1.0".to_owned()),
            vec![output("aa", &[])],
        ),
        scenario("b", &["-i", "y"], Value::Err("no".to_owned()), Vec::new()),
    ]);
    let mut actual = expected.clone();
    actual.scenarios[0].calls[0].args[1] = "z".to_owned();
    actual.scenarios[0]
        .files
        .push("{work}/a/list.txt".to_owned());
    actual.scenarios[0].result = Value::Ok("1.1".to_owned());
    actual.scenarios[0].outputs[0].sha256 = "bb".to_owned();
    // Error messages may differ: only failing matters.
    actual.scenarios[1].result = Value::Err("another message".to_owned());
    let problems = compare(&expected, &actual);
    assert_eq!(problems.len(), 4, "{problems:#?}");
    assert!(
        problems[3].contains("decoded it is the same"),
        "{problems:#?}"
    );
    actual.scenarios[1].result = Value::Ok(String::new());
    assert_eq!(compare(&expected, &actual).len(), 5);
}

#[test]
fn room_tone_and_what_reads_it_agree_within_the_noise() {
    let expected = reference(vec![
        scenario(
            "tone",
            &["-i", "anoisesrc=color=pink"],
            Value::Ok(String::new()),
            vec![output("aa", &["stream|codec_name=mp3|bit_rate=50240"])],
        ),
        scenario(
            "measure",
            &["-i", "{tone#0}"],
            Value::Ok("floor: -69.40".to_owned()),
            Vec::new(),
        ),
        scenario(
            "plain",
            &["-i", "x"],
            Value::Ok("floor: -69.40".to_owned()),
            Vec::new(),
        ),
    ]);
    assert_eq!(random_scenarios(&expected), vec!["tone", "measure"]);
    let mut actual = expected.clone();
    actual.scenarios[0].outputs[0].sha256 = "bb".to_owned();
    actual.scenarios[0].outputs[0].streams[0] = "stream|codec_name=mp3|bit_rate=49920".to_owned();
    actual.scenarios[1].result = Value::Ok("floor: -69.97".to_owned());
    assert!(compare(&expected, &actual).is_empty());
    actual.scenarios[0].outputs[0].streams[0] = "stream|codec_name=aac|bit_rate=1".to_owned();
    actual.scenarios[2].result = Value::Ok("floor: -69.97".to_owned());
    assert_eq!(compare(&expected, &actual).len(), 2);
}

#[test]
fn other_scenarios_are_named() {
    let expected = reference(vec![scenario(
        "a",
        &[],
        Value::Ok(String::new()),
        Vec::new(),
    )]);
    let actual = reference(vec![scenario(
        "b",
        &[],
        Value::Ok(String::new()),
        Vec::new(),
    )]);
    let problems = compare(&expected, &actual);
    assert!(problems[0].contains("the scenarios differ"), "{problems:?}");
}
