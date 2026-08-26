use std::collections::VecDeque;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const EV_KEY: u16 = 0x01;
const KEY_ESC: u16 = 1;
const KEY_1: u16 = 2;
const KEY_0: u16 = 11;
const KEY_BACKSPACE: u16 = 14;
const KEY_TAB: u16 = 15;
const KEY_Q: u16 = 16;
const KEY_P: u16 = 25;
const KEY_ENTER: u16 = 28;
const KEY_A: u16 = 30;
const KEY_L: u16 = 38;
const KEY_Z: u16 = 44;
const KEY_M: u16 = 50;
const KEY_SPACE: u16 = 57;
const KEY_MINUS: u16 = 12;
const KEY_EQUAL: u16 = 13;
const KEY_LEFTBRACE: u16 = 26;
const KEY_RIGHTBRACE: u16 = 27;
const KEY_SEMICOLON: u16 = 39;
const KEY_APOSTROPHE: u16 = 40;
const KEY_GRAVE: u16 = 41;
const KEY_BACKSLASH: u16 = 43;
const KEY_COMMA: u16 = 51;
const KEY_DOT: u16 = 52;
const KEY_SLASH: u16 = 53;

#[derive(Clone, Copy, Debug)]
struct Config {
    window: Duration,
    idle_timeout: Duration,
    word_len: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window: Duration::from_millis(2_000),
            idle_timeout: Duration::from_millis(900),
            word_len: 4.8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct KeyPress {
    at: Instant,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("wpm-rt: {err}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let (mode, config) = parse_args()?;

    match mode.as_deref() {
        None | Some("stream") => stream(config),
        Some("devices") => {
            for path in input_devices()? {
                println!("{}", path.display());
            }
            Ok(())
        }
        Some("-h") | Some("--help") | Some("help") => {
            print_help();
            Ok(())
        }
        Some(other) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown command `{other}`"),
        )),
    }
}

fn parse_args() -> io::Result<(Option<String>, Config)> {
    let mut args = env::args().skip(1).peekable();
    let mut mode = None;
    let mut config = Config::default();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--window-ms" => {
                config.window = Duration::from_millis(parse_next(&mut args, "--window-ms")?);
            }
            "--idle-ms" => {
                config.idle_timeout = Duration::from_millis(parse_next(&mut args, "--idle-ms")?);
            }
            "--word-len" => {
                config.word_len = parse_next_f64(&mut args, "--word-len")?;
                if config.word_len <= 0.0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--word-len must be greater than zero",
                    ));
                }
            }
            flag if flag.starts_with('-') => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown flag `{flag}`"),
                ));
            }
            command => {
                if mode.replace(command.to_string()).is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "only one command can be provided",
                    ));
                }
            }
        }
    }

    Ok((mode, config))
}

fn parse_next<I>(args: &mut I, flag: &str) -> io::Result<u64>
where
    I: Iterator<Item = String>,
{
    let value = args.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{flag} needs a value"))
    })?;
    value.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} needs an integer value"),
        )
    })
}

fn parse_next_f64<I>(args: &mut I, flag: &str) -> io::Result<f64>
where
    I: Iterator<Item = String>,
{
    let value = args.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{flag} needs a value"))
    })?;
    value.parse::<f64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} needs a numeric value"),
        )
    })
}

fn print_help() {
    println!(
        "wpm-rt\n\nCommands:\n  stream   emit live WPM updates as newline-delimited JSON\n  devices  list readable input event devices\n\nOptions:\n  --window-ms N   rolling WPM window in milliseconds, default 2000\n  --idle-ms N     inactive timeout in milliseconds, default 900\n  --word-len N    average characters per word, default 4.8"
    );
}

fn stream(config: Config) -> io::Result<()> {
    let devices = input_devices()?;
    if devices.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "no readable /dev/input/event* devices found; try adding your user to the input group",
        ));
    }

    let (tx, rx) = mpsc::channel();
    for path in devices {
        let tx = tx.clone();
        thread::spawn(move || {
            let _ = read_input_device(&path, tx);
        });
    }
    drop(tx);

    emit_stats(rx, config)
}

fn input_devices() -> io::Result<Vec<PathBuf>> {
    let mut devices = Vec::new();
    for entry in fs::read_dir("/dev/input")? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("event") {
            continue;
        }
        if is_keyboardish(name) && File::open(&path).is_ok() {
            devices.push(path);
        }
    }

    devices.sort();
    Ok(devices)
}

fn is_keyboardish(event_name: &str) -> bool {
    let capability_path = Path::new("/sys/class/input")
        .join(event_name)
        .join("device/capabilities/key");

    let Ok(keys) = fs::read_to_string(capability_path) else {
        return true;
    };

    let bitmap = parse_hex_bitmap(&keys);
    has_key(&bitmap, KEY_A) && has_key(&bitmap, KEY_SPACE)
}

fn parse_hex_bitmap(input: &str) -> Vec<u64> {
    input
        .split_whitespace()
        .rev()
        .filter_map(|part| u64::from_str_radix(part, 16).ok())
        .collect()
}

fn has_key(bitmap: &[u64], code: u16) -> bool {
    let index = usize::from(code / 64);
    let bit = u32::from(code % 64);
    bitmap
        .get(index)
        .map(|word| (word & (1_u64 << bit)) != 0)
        .unwrap_or(false)
}

fn read_input_device(path: &Path, tx: mpsc::Sender<KeyPress>) -> io::Result<()> {
    let mut file = File::open(path)?;
    let mut event = [0_u8; 24];

    loop {
        file.read_exact(&mut event)?;
        let event_type = u16::from_ne_bytes([event[16], event[17]]);
        let code = u16::from_ne_bytes([event[18], event[19]]);
        let value = i32::from_ne_bytes([event[20], event[21], event[22], event[23]]);

        if event_type == EV_KEY && value == 1 && is_typing_key(code) {
            if tx.send(KeyPress { at: Instant::now() }).is_err() {
                return Ok(());
            }
        }
    }
}

fn is_typing_key(code: u16) -> bool {
    matches!(
        code,
        KEY_1..=KEY_0
            | KEY_Q..=KEY_P
            | KEY_A..=KEY_L
            | KEY_Z..=KEY_M
            | KEY_SPACE
            | KEY_TAB
            | KEY_ENTER
            | KEY_MINUS
            | KEY_EQUAL
            | KEY_LEFTBRACE
            | KEY_RIGHTBRACE
            | KEY_SEMICOLON
            | KEY_APOSTROPHE
            | KEY_GRAVE
            | KEY_BACKSLASH
            | KEY_COMMA
            | KEY_DOT
            | KEY_SLASH
    ) && code != KEY_ESC
        && code != KEY_BACKSPACE
}

fn emit_stats(rx: Receiver<KeyPress>, config: Config) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    let mut events = VecDeque::new();
    let tick = Duration::from_millis(100);
    let mut last_active = Instant::now();
    let mut was_active = false;

    loop {
        match rx.recv_timeout(tick) {
            Ok(key) => {
                last_active = key.at;
                events.push_back(key.at);
                drain_old(&mut events, key.at, config.window);
                write_stats(&mut stdout, &events, config, true)?;
                was_active = true;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                drain_old(&mut events, now, config.window);
                let active = now.duration_since(last_active) <= config.idle_timeout;

                if active && !events.is_empty() {
                    write_stats(&mut stdout, &events, config, true)?;
                    was_active = true;
                } else if was_active {
                    events.clear();
                    write_stats(&mut stdout, &events, config, false)?;
                    was_active = false;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn drain_old(events: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while events
        .front()
        .map(|at| now.duration_since(*at) > window)
        .unwrap_or(false)
    {
        events.pop_front();
    }
}

fn write_stats(
    writer: &mut impl Write,
    events: &VecDeque<Instant>,
    config: Config,
    active: bool,
) -> io::Result<()> {
    let cps = if events.len() >= 2 {
        let span = events.back().unwrap().duration_since(*events.front().unwrap());
        let seconds = span.as_secs_f64().max(0.25);
        events.len() as f64 / seconds
    } else {
        0.0
    };

    let wpm = cps * 60.0 / config.word_len;
    writeln!(
        writer,
        "{{\"wpm\":{},\"cps\":{:.2},\"active\":{}}}",
        wpm.round() as u64,
        cps,
        active
    )?;
    writer.flush()
}
