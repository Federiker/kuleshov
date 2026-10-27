use super::*;
use std::sync::Arc;
use std::{fmt::Write as _, time::Instant};

fn shell(script: &str) -> Command {
    if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", script]);
        command
    } else {
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        command
    }
}

#[test]
fn cancellation_stops_a_running_child_process() {
    let cancel = Arc::new(AtomicBool::new(false));
    let thread_flag = cancel.clone();
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        thread_flag.store(true, Ordering::Relaxed);
    });
    let started = Instant::now();
    let script = if cfg!(windows) {
        "ping -n 30 127.0.0.1 >NUL"
    } else {
        "sleep 30"
    };
    let result = run_cancellable(&mut shell(script), Some(&cancel));
    setter.join().unwrap();

    assert!(result.unwrap_err().is_cancelled());
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn an_already_cancelled_process_is_never_started() {
    let cancel = AtomicBool::new(true);
    // A program that does not exist: starting it would be an error of its own.
    let mut command = Command::new("kuleshov-not-a-real-tool");

    let error = run_cancellable(&mut command, Some(&cancel)).unwrap_err();
    assert!(matches!(error, Error::Cancelled));
}

#[test]
fn a_tool_that_cannot_start_is_named() {
    let error = run_cancellable(&mut Command::new("kuleshov-not-a-real-tool"), None).unwrap_err();
    assert!(matches!(error, Error::Run { .. }), "{error:?}");
    assert!(
        error
            .to_string()
            .starts_with("could not run kuleshov-not-a-real-tool: "),
        "{error}"
    );
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn a_failed_tool_reports_its_exit_code_and_last_error_lines() {
    let error = run_cancellable(
        &mut shell("echo first 1>&2 && echo second 1>&2 && exit 3"),
        None,
    )
    .unwrap_err();

    let Error::Failed {
        code, stderr, file, ..
    } = &error
    else {
        panic!("expected a failure, got {error:?}");
    };
    assert_eq!(*code, Some(3));
    assert_eq!(stderr, "first / second");
    assert_eq!(*file, None);
    let program = if cfg!(windows) { "cmd" } else { "sh" };
    assert_eq!(
        error.to_string(),
        format!("{program} exited with code 3: first / second")
    );
}

#[test]
fn a_successful_tool_returns_its_status() {
    let status = run_cancellable(&mut shell("echo noise 1>&2"), None).unwrap();
    assert!(status.success());
}

#[test]
fn the_error_tail_keeps_the_last_lines_and_splits_progress_updates() {
    let lines = (1..=12).fold(String::new(), |mut lines, n| {
        let _ = writeln!(lines, "line {n}");
        lines
    });
    assert_eq!(
        stderr_tail(lines.as_bytes()),
        "line 5 / line 6 / line 7 / line 8 / line 9 / line 10 / line 11 / line 12"
    );
    assert_eq!(
        stderr_tail(b"frame=1\rframe=2\r\n\nError opening input\n"),
        "frame=1 / frame=2 / Error opening input"
    );
    assert_eq!(stderr_tail(b""), "");
}

#[test]
fn output_names_the_file_of_a_failed_run() {
    let file = Path::new("chapter-09.wav");
    let error = output(&mut shell("echo broken 1>&2 && exit 2"), file).unwrap_err();
    assert_eq!(error.file(), Some(file));
    assert!(error.to_string().contains("chapter-09.wav"), "{error}");
    assert!(
        error.to_string().ends_with("exited with code 2: broken"),
        "{error}"
    );
    let error = output(&mut Command::new("kuleshov-not-a-real-tool"), file).unwrap_err();
    assert!(matches!(error, Error::Run { .. }), "{error:?}");
}
