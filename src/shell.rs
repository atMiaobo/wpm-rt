use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const QML_TEMPLATE: &str = include_str!("../quickshell/shell.qml");

fn main() {
    if let Err(err) = run() {
        eprintln!("wpm-rt-shell: {err}");
        process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let quickshell = find_program("quickshell").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "`quickshell` was not found in PATH; install Quickshell first",
        )
    })?;

    let daemon = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("wpm-rt")))
        .filter(|path| path.exists())
        .or_else(|| find_program("wpm-rt"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "`wpm-rt` was not found next to `wpm-rt-shell` or in PATH",
            )
        })?;

    let qml_path = runtime_dir()?.join("wpm-rt").join("shell.qml");
    let qml = QML_TEMPLATE.replace("@wpmRtBin@", &daemon.to_string_lossy());

    if let Some(parent) = qml_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&qml_path, qml)?;

    let status = Command::new(quickshell).arg("-p").arg(qml_path).status()?;
    process::exit(status.code().unwrap_or(1));
}

fn runtime_dir() -> io::Result<PathBuf> {
    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(dir));
    }

    Ok(env::temp_dir())
}

fn find_program(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
}
