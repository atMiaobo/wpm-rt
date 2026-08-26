use std::collections::{BTreeMap, HashSet, VecDeque};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EV_KEY: u16 = 0x01;
const KEY_ESC: u16 = 1;
const KEY_1: u16 = 2;
const KEY_0: u16 = 11;
const KEY_BACKSPACE: u16 = 14;
const KEY_TAB: u16 = 15;
const KEY_LEFTCTRL: u16 = 29;
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
const KEY_LEFTALT: u16 = 56;
const KEY_LEFTMETA: u16 = 125;
const KEY_RIGHTMETA: u16 = 126;
const KEY_RIGHTCTRL: u16 = 97;
const KEY_RIGHTALT: u16 = 100;

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
        Some("stats") => serve_stats(),
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
        "wpm-rt\n\nCommands:\n  stream   emit live WPM updates as newline-delimited JSON and record sessions\n  devices  list readable input event devices\n  stats    open a localhost stats dashboard\n\nOptions:\n  --window-ms N   rolling WPM window in milliseconds, default 2000\n  --idle-ms N     inactive timeout in milliseconds, default 900\n  --word-len N    average characters per word, default 4.8"
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
    let mut shortcut_modifiers_down = HashSet::new();

    loop {
        file.read_exact(&mut event)?;
        let event_type = u16::from_ne_bytes([event[16], event[17]]);
        let code = u16::from_ne_bytes([event[18], event[19]]);
        let value = i32::from_ne_bytes([event[20], event[21], event[22], event[23]]);

        if event_type == EV_KEY {
            if is_shortcut_modifier(code) {
                if value == 0 {
                    shortcut_modifiers_down.remove(&code);
                } else {
                    shortcut_modifiers_down.insert(code);
                }
            }

            if value == 1 && shortcut_modifiers_down.is_empty() && is_typing_key(code) {
                if tx.send(KeyPress { at: Instant::now() }).is_err() {
                    return Ok(());
                }
            }
        }
    }
}

fn is_shortcut_modifier(code: u16) -> bool {
    matches!(
        code,
        KEY_LEFTCTRL | KEY_RIGHTCTRL | KEY_LEFTALT | KEY_RIGHTALT | KEY_LEFTMETA | KEY_RIGHTMETA
    )
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
    let mut recorder = SessionRecorder::new()?;

    loop {
        match rx.recv_timeout(tick) {
            Ok(key) => {
                last_active = key.at;
                events.push_back(key.at);
                drain_old(&mut events, key.at, config.window);
                let stats = write_stats(&mut stdout, &events, config, key.at, true)?;
                recorder.record(key.at, stats)?;
                was_active = true;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                drain_old(&mut events, now, config.window);
                let active = now.duration_since(last_active) <= config.idle_timeout;

                if active && !events.is_empty() {
                    let stats = write_stats(&mut stdout, &events, config, now, true)?;
                    recorder.record(now, stats)?;
                    was_active = true;
                } else if was_active {
                    events.clear();
                    write_stats(&mut stdout, &events, config, now, false)?;
                    recorder.end();
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
    now: Instant,
    active: bool,
) -> io::Result<LiveStats> {
    let cps = if let Some(first) = events.front() {
        let span = now.duration_since(*first);
        let seconds = span
            .as_secs_f64()
            .max(1.0)
            .min(config.window.as_secs_f64());
        events.len() as f64 / seconds
    } else {
        0.0
    };

    let wpm = cps * 60.0 / config.word_len;
    let stats = LiveStats {
        wpm: wpm.round() as u64,
        cps,
    };
    writeln!(
        writer,
        "{{\"wpm\":{},\"cps\":{:.2},\"active\":{}}}",
        stats.wpm,
        stats.cps,
        active
    )?;
    writer.flush()?;
    Ok(stats)
}

#[derive(Clone, Copy, Debug)]
struct LiveStats {
    wpm: u64,
    cps: f64,
}

#[derive(Debug)]
struct SessionRecorder {
    file: File,
    session_id: Option<String>,
    last_recorded_at: Option<Instant>,
}

impl SessionRecorder {
    fn new() -> io::Result<Self> {
        let dir = data_dir()?;
        fs::create_dir_all(&dir)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("samples.tsv"))?;

        Ok(Self {
            file,
            session_id: None,
            last_recorded_at: None,
        })
    }

    fn record(&mut self, now: Instant, stats: LiveStats) -> io::Result<()> {
        if stats.wpm == 0 {
            return Ok(());
        }

        if self
            .last_recorded_at
            .map(|last| now.duration_since(last) < Duration::from_millis(250))
            .unwrap_or(false)
        {
            return Ok(());
        }

        if self.session_id.is_none() {
            self.session_id = Some(format!("{}", epoch_millis()));
        }
        let session_id = self.session_id.clone().unwrap_or_default();
        self.last_recorded_at = Some(now);

        writeln!(
            self.file,
            "{}\t{}\t{}\t{:.2}",
            session_id,
            epoch_millis(),
            stats.wpm,
            stats.cps
        )?;
        self.file.flush()
    }

    fn end(&mut self) {
        self.session_id = None;
        self.last_recorded_at = None;
    }
}

#[derive(Clone, Debug)]
struct Sample {
    session_id: String,
    epoch_ms: u128,
    wpm: u64,
    cps: f64,
}

#[derive(Clone, Debug)]
struct SessionStats {
    session_id: String,
    start_ms: u128,
    end_ms: u128,
    avg_wpm: f64,
    max_wpm: u64,
    min_wpm: u64,
    sample_count: usize,
}

fn data_dir() -> io::Result<PathBuf> {
    if let Some(dir) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(dir).join("wpm-rt"));
    }

    let home = env::var_os("HOME").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "HOME is not set and XDG_DATA_HOME was not provided",
        )
    })?;
    Ok(PathBuf::from(home).join(".local/share/wpm-rt"))
}

fn epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn serve_stats() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8787")
        .or_else(|_| TcpListener::bind("127.0.0.1:0"))?;
    let addr = listener.local_addr()?;
    let url = format!("http://{addr}");

    println!("wpm-rt stats: {url}");
    let _ = Command::new("xdg-open").arg(&url).spawn();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let _ = handle_stats_request(stream);
            }
            Err(err) => eprintln!("wpm-rt stats: {err}"),
        }
    }

    Ok(())
}

fn handle_stats_request(mut stream: TcpStream) -> io::Result<()> {
    let mut first_line = String::new();
    {
        let mut reader = BufReader::new(&mut stream);
        reader.read_line(&mut first_line)?;
    }

    let samples = read_samples()?;
    let html = stats_html(&samples);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    stream.write_all(response.as_bytes())
}

fn read_samples() -> io::Result<Vec<Sample>> {
    let path = data_dir()?.join("samples.tsv");
    let Ok(file) = File::open(path) else {
        return Ok(Vec::new());
    };

    let mut samples = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let parts: Vec<_> = line.split('\t').collect();
        if parts.len() != 4 {
            continue;
        }

        let Ok(epoch_ms) = parts[1].parse::<u128>() else {
            continue;
        };
        let Ok(wpm) = parts[2].parse::<u64>() else {
            continue;
        };
        let Ok(cps) = parts[3].parse::<f64>() else {
            continue;
        };

        samples.push(Sample {
            session_id: parts[0].to_string(),
            epoch_ms,
            wpm,
            cps,
        });
    }

    Ok(samples)
}

fn summarize_sessions(samples: &[Sample]) -> Vec<SessionStats> {
    let mut by_session: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
    for sample in samples {
        by_session
            .entry(sample.session_id.as_str())
            .or_default()
            .push(sample);
    }

    let mut sessions = Vec::new();
    for (session_id, mut session_samples) in by_session {
        session_samples.sort_by_key(|sample| sample.epoch_ms);
        let sample_count = session_samples.len();
        if sample_count == 0 {
            continue;
        }

        let start_ms = session_samples.first().unwrap().epoch_ms;
        let end_ms = session_samples.last().unwrap().epoch_ms;
        let total_wpm: u64 = session_samples.iter().map(|sample| sample.wpm).sum();
        let max_wpm = session_samples
            .iter()
            .map(|sample| sample.wpm)
            .max()
            .unwrap_or(0);
        let min_wpm = session_samples
            .iter()
            .map(|sample| sample.wpm)
            .min()
            .unwrap_or(0);

        sessions.push(SessionStats {
            session_id: session_id.to_string(),
            start_ms,
            end_ms,
            avg_wpm: total_wpm as f64 / sample_count as f64,
            max_wpm,
            min_wpm,
            sample_count,
        });
    }

    sessions.sort_by_key(|session| std::cmp::Reverse(session.start_ms));
    sessions
}

fn stats_html(samples: &[Sample]) -> String {
    let sessions = summarize_sessions(samples);
    let mut ordered_samples = samples.to_vec();
    ordered_samples.sort_by_key(|sample| sample.epoch_ms);
    let samples_json = samples_json(&ordered_samples);
    let sessions_json = sessions_json(&sessions);

    format!(
        r##"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>wpm-rt stats</title>
  <style>
    :root {{ color-scheme: dark; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }}
    body {{ margin: 0; background: #111; color: #e8e3d8; }}
    main {{ max-width: 1120px; margin: 0 auto; padding: 24px; }}
    header {{ display: flex; align-items: end; justify-content: space-between; gap: 16px; margin-bottom: 20px; }}
    h1 {{ margin: 0; font-size: 20px; font-weight: 600; }}
    .muted {{ color: #a9a39a; font-size: 12px; }}
    .toolbar {{ display: flex; gap: 8px; flex-wrap: wrap; margin-bottom: 16px; }}
    button {{ background: #181818; color: #e8e3d8; border: 1px solid #d8d3c8; padding: 7px 10px; font: inherit; cursor: pointer; }}
    button.active {{ background: #e8e3d8; color: #111; }}
    .grid {{ display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; margin-bottom: 16px; }}
    .stat {{ border: 1px solid #393632; padding: 12px; background: #151515; }}
    .label {{ color: #a9a39a; font-size: 11px; margin-bottom: 6px; }}
    .value {{ font-size: 24px; }}
    canvas {{ width: 100%; height: 360px; border: 1px solid #393632; background: #151515; display: block; }}
    table {{ width: 100%; border-collapse: collapse; margin-top: 18px; font-size: 12px; }}
    th, td {{ border-bottom: 1px solid #2f2c29; padding: 8px; text-align: left; }}
    th {{ color: #a9a39a; font-weight: 500; }}
    tr {{ cursor: pointer; }}
    tr.active {{ background: #24211d; }}
    @media (max-width: 720px) {{
      main {{ padding: 14px; }}
      header {{ display: block; }}
      .grid {{ grid-template-columns: repeat(2, minmax(0, 1fr)); }}
      canvas {{ height: 300px; }}
    }}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>wpm-rt stats</h1>
        <div class="muted" id="subtitle"></div>
      </div>
      <div class="muted">data: {data_path}</div>
    </header>

    <div class="toolbar">
      <button id="allButton" class="active">all time</button>
      <button id="latestButton">latest session</button>
    </div>

    <div class="grid">
      <div class="stat"><div class="label">average</div><div class="value" id="avg">0</div></div>
      <div class="stat"><div class="label">max</div><div class="value" id="max">0</div></div>
      <div class="stat"><div class="label">min</div><div class="value" id="min">0</div></div>
      <div class="stat"><div class="label">samples</div><div class="value" id="count">0</div></div>
    </div>

    <canvas id="chart" width="1100" height="360"></canvas>
    <table>
      <thead><tr><th>session</th><th>date</th><th>avg</th><th>max</th><th>min</th><th>samples</th></tr></thead>
      <tbody id="sessions"></tbody>
    </table>
  </main>

  <script>
    const samples = {samples_json};
    const sessions = {sessions_json};
    let selected = "all";

    const byId = id => document.getElementById(id);
    const fmtDate = ms => new Date(Number(ms)).toLocaleString();
    const round = n => Math.round(n);

    function filteredSamples() {{
      if (selected === "all") return samples;
      return samples.filter(sample => sample.session_id === selected);
    }}

    function renderStats(rows) {{
      if (!rows.length) {{
        byId("avg").textContent = "0";
        byId("max").textContent = "0";
        byId("min").textContent = "0";
        byId("count").textContent = "0";
        return;
      }}
      const wpms = rows.map(row => row.wpm);
      byId("avg").textContent = round(wpms.reduce((a, b) => a + b, 0) / wpms.length);
      byId("max").textContent = Math.max(...wpms);
      byId("min").textContent = Math.min(...wpms);
      byId("count").textContent = rows.length;
    }}

    function renderChart(rows) {{
      const canvas = byId("chart");
      const ctx = canvas.getContext("2d");
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);
      ctx.fillStyle = "#151515";
      ctx.fillRect(0, 0, w, h);

      if (!rows.length) {{
        ctx.fillStyle = "#a9a39a";
        ctx.font = "14px monospace";
        ctx.fillText("no samples yet", 24, 36);
        return;
      }}

      const pad = 34;
      const maxWpm = Math.max(20, ...rows.map(row => row.wpm));
      const minTime = Number(rows[0].epoch_ms);
      const maxTime = Number(rows[rows.length - 1].epoch_ms);
      const span = Math.max(1, maxTime - minTime);

      ctx.strokeStyle = "#393632";
      ctx.lineWidth = 1;
      for (let i = 0; i <= 4; i++) {{
        const y = pad + ((h - pad * 2) * i / 4);
        ctx.beginPath();
        ctx.moveTo(pad, y);
        ctx.lineTo(w - pad, y);
        ctx.stroke();
      }}

      ctx.strokeStyle = "#e8e3d8";
      ctx.lineWidth = 2;
      ctx.beginPath();
      rows.forEach((row, i) => {{
        const x = pad + ((Number(row.epoch_ms) - minTime) / span) * (w - pad * 2);
        const y = h - pad - (row.wpm / maxWpm) * (h - pad * 2);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }});
      ctx.stroke();

      ctx.fillStyle = "#a9a39a";
      ctx.font = "12px monospace";
      ctx.fillText(maxWpm + " wpm", pad, 18);
      ctx.fillText("0", pad, h - 10);
    }}

    function renderTable() {{
      byId("sessions").innerHTML = sessions.map(session =>
        '<tr class="' + (selected === session.session_id ? 'active' : '') + '" data-id="' + session.session_id + '">' +
          '<td>' + session.session_id + '</td>' +
          '<td>' + fmtDate(session.start_ms) + '</td>' +
          '<td>' + round(session.avg_wpm) + '</td>' +
          '<td>' + session.max_wpm + '</td>' +
          '<td>' + session.min_wpm + '</td>' +
          '<td>' + session.sample_count + '</td>' +
        '</tr>'
      ).join("");

      document.querySelectorAll("tr[data-id]").forEach(row => {{
        row.addEventListener("click", () => {{
          selected = row.dataset.id;
          render();
        }});
      }});
    }}

    function render() {{
      const rows = filteredSamples();
      byId("allButton").classList.toggle("active", selected === "all");
      byId("latestButton").classList.toggle("active", selected === (sessions[0] && sessions[0].session_id));
      byId("subtitle").textContent = selected === "all" ? "all sessions combined" : "session " + selected;
      renderStats(rows);
      renderChart(rows);
      renderTable();
    }}

    byId("allButton").addEventListener("click", () => {{ selected = "all"; render(); }});
    byId("latestButton").addEventListener("click", () => {{
      if (sessions[0]) selected = sessions[0].session_id;
      render();
    }});

    render();
  </script>
</body>
</html>"##,
        data_path = html_escape(&data_dir().unwrap_or_default().display().to_string()),
        samples_json = samples_json,
        sessions_json = sessions_json
    )
}

fn samples_json(samples: &[Sample]) -> String {
    let mut out = String::from("[");
    for (index, sample) in samples.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"session_id\":\"{}\",\"epoch_ms\":{},\"wpm\":{},\"cps\":{:.2}}}",
            json_escape(&sample.session_id),
            sample.epoch_ms,
            sample.wpm,
            sample.cps
        ));
    }
    out.push(']');
    out
}

fn sessions_json(sessions: &[SessionStats]) -> String {
    let mut out = String::from("[");
    for (index, session) in sessions.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"session_id\":\"{}\",\"start_ms\":{},\"end_ms\":{},\"avg_wpm\":{:.2},\"max_wpm\":{},\"min_wpm\":{},\"sample_count\":{}}}",
            json_escape(&session.session_id),
            session.start_ms,
            session.end_ms,
            session.avg_wpm,
            session.max_wpm,
            session.min_wpm,
            session.sample_count
        ));
    }
    out.push(']');
    out
}

fn json_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
