use super::*;
use std::fs;

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kuleshov-tools-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn finds_a_tool_in_the_first_folder_that_has_it() {
    let first = scratch("first");
    let second = scratch("second");
    fs::write(second.join(Tool::Ffmpeg.file_name()), b"").unwrap();
    let found = find_in(Tool::Ffmpeg, &[first.clone(), second.clone()]).unwrap();
    assert_eq!(found, second.join(Tool::Ffmpeg.file_name()));
    fs::write(first.join(Tool::Ffmpeg.file_name()), b"").unwrap();
    let found = find_in(Tool::Ffmpeg, &[first.clone(), second.clone()]).unwrap();
    assert_eq!(found, first.join(Tool::Ffmpeg.file_name()));
    let _ = fs::remove_dir_all(first);
    let _ = fs::remove_dir_all(second);
}

#[test]
fn each_tool_is_found_on_its_own_and_falls_back_to_path() {
    let dir = scratch("one");
    fs::write(dir.join(Tool::Ffprobe.file_name()), b"").unwrap();
    // A folder is no tool.
    fs::create_dir_all(dir.join(Tool::Ffmpeg.file_name())).unwrap();
    let tools = Tools::find_in([&dir]);
    assert_eq!(tools.path(Tool::Ffmpeg), PathBuf::from("ffmpeg"));
    assert_eq!(
        tools.path(Tool::Ffprobe),
        dir.join(Tool::Ffprobe.file_name())
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_default_runs_the_bare_names_from_path() {
    for tools in [Tools::default(), Tools::from_path()] {
        assert_eq!(tools.path(Tool::Ffmpeg), PathBuf::from("ffmpeg"));
        assert_eq!(tools.path(Tool::Ffprobe), PathBuf::from("ffprobe"));
        assert_eq!(format!("{tools:?}"), "Tools(PATH)");
    }
}

#[test]
fn explicit_paths_are_run_as_given() {
    let tools = Tools::new("/opt/ff/ffmpeg", "/opt/fp/ffprobe");
    assert_eq!(tools.path(Tool::Ffmpeg), PathBuf::from("/opt/ff/ffmpeg"));
    assert_eq!(tools.path(Tool::Ffprobe), PathBuf::from("/opt/fp/ffprobe"));
    assert_eq!(
        tools.command(Tool::Ffprobe).get_program(),
        std::ffi::OsStr::new("/opt/fp/ffprobe")
    );
    assert!(format!("{tools:?}").contains("/opt/ff/ffmpeg"));
}

#[test]
fn a_resolver_is_asked_each_time_and_may_leave_a_tool_to_path() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = asked.clone();
    let tools = Tools::with_resolver(move |tool| {
        counter.fetch_add(1, Ordering::Relaxed);
        (tool == Tool::Ffmpeg).then(|| PathBuf::from("/staged/ffmpeg"))
    });
    assert_eq!(tools.path(Tool::Ffmpeg), PathBuf::from("/staged/ffmpeg"));
    assert_eq!(tools.path(Tool::Ffprobe), PathBuf::from("ffprobe"));
    let _ = tools.clone().command(Tool::Ffmpeg);
    assert_eq!(asked.load(Ordering::Relaxed), 3);
    assert_eq!(format!("{tools:?}"), "Tools(resolver)");
}

#[test]
fn tools_have_their_names() {
    assert_eq!(Tool::Ffmpeg.to_string(), "ffmpeg");
    assert_eq!(Tool::Ffprobe.name(), "ffprobe");
    assert_eq!(
        Tool::Ffprobe.file_name(),
        if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        }
    );
}

#[cfg(windows)]
const CONSOLE_PROBE: &str = "KULESHOV_CONSOLE_PROBE";

#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "asks Windows whether this process has a console window"
)]
fn has_console_window() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
    }
    // SAFETY: no arguments; returns the console window handle or null.
    !unsafe { GetConsoleWindow() }.is_null()
}

/// Passes on its own; started as a child by the next test, it reports whether it has a console window.
#[cfg(windows)]
#[test]
fn console_probe() {
    if std::env::var_os(CONSOLE_PROBE).is_some() {
        println!("console-window={}", has_console_window());
    }
}

#[cfg(windows)]
fn probe(hidden: bool) -> String {
    let mut command = Command::new(std::env::current_exe().unwrap());
    if hidden {
        hide_console(&mut command);
    }
    let output = command
        .args(["--exact", "tools::tests::console_probe", "--nocapture"])
        .env(CONSOLE_PROBE, "1")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[cfg(windows)]
#[test]
fn tools_start_without_a_console_window() {
    let hidden = probe(true);
    assert!(hidden.contains("console-window=false"), "{hidden}");
    // Without the flag a child shares this test's console, so the probe can tell the difference.
    if has_console_window() {
        let shown = probe(false);
        assert!(shown.contains("console-window=true"), "{shown}");
    }
}
