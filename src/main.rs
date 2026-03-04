use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;
use xxhash_rust::xxh3::Xxh3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

impl LogLevel {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" | "warning" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            other => Err(format!("invalid --log-level value: {other}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
        }
    }
}

#[derive(Debug, Clone)]
struct Config {
    root: PathBuf,
    cmd: String,
    shell: ShellKind,
    workdir: PathBuf,
    exts: HashSet<String>,
    debounce: Duration,
    reconcile_interval: Duration,
    log_level: LogLevel,
}

#[derive(Debug, Clone, Copy)]
enum ShellKind {
    Sh,
    Bash,
    Cmd,
    PowerShell,
}

impl ShellKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "sh" => Ok(Self::Sh),
            "bash" => Ok(Self::Bash),
            "cmd" => Ok(Self::Cmd),
            "powershell" | "pwsh" => Ok(Self::PowerShell),
            other => Err(format!("invalid --shell value: {other}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Sh => "sh",
            Self::Bash => "bash",
            Self::Cmd => "cmd",
            Self::PowerShell => "powershell",
        }
    }
}

#[derive(Debug)]
enum ParseOutcome {
    Help,
    Config(Config),
}

impl Config {
    fn parse() -> Result<ParseOutcome, String> {
        let mut root: Option<PathBuf> = None;
        let mut cmd = default_cmd();
        let mut shell = default_shell();
        let mut workdir = env::current_dir().map_err(|e| format!("get current dir failed: {e}"))?;
        let mut ext_csv = String::from("md,markdown");
        let mut debounce_ms: u64 = 800;
        let mut reconcile_sec: u64 = 600;
        let mut log_level = LogLevel::Info;

        let mut args = env::args().skip(1).peekable();
        if args.peek().is_none() {
            return Err(usage_text());
        }

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Ok(ParseOutcome::Help),
                "--root" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --root"))?;
                    root = Some(PathBuf::from(value));
                }
                "--cmd" => {
                    cmd = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --cmd"))?;
                }
                "--shell" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --shell"))?;
                    shell = ShellKind::parse(&value)?;
                }
                "--workdir" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --workdir"))?;
                    workdir = PathBuf::from(value);
                }
                "--ext" => {
                    ext_csv = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --ext"))?;
                }
                "--debounce-ms" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --debounce-ms"))?;
                    debounce_ms = parse_u64_arg("--debounce-ms", &value)?;
                }
                "--reconcile-sec" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --reconcile-sec"))?;
                    reconcile_sec = parse_u64_arg("--reconcile-sec", &value)?;
                }
                "--log-level" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --log-level"))?;
                    log_level = LogLevel::parse(&value)?;
                }
                other => {
                    return Err(format!("unknown argument: {other}\n\n{}", usage_text()));
                }
            }
        }

        let root = root.ok_or_else(|| String::from("missing required --root argument"))?;
        let cwd = env::current_dir().map_err(|e| format!("get current dir failed: {e}"))?;
        let root = absolutize(&cwd, &root);
        let workdir = absolutize(&cwd, &workdir);
        if !root.is_dir() {
            return Err(format!("--root is not a directory: {}", root.display()));
        }
        if !workdir.is_dir() {
            return Err(format!(
                "--workdir is not a directory: {}",
                workdir.display()
            ));
        }

        let exts = parse_extensions(&ext_csv)?;
        if exts.is_empty() {
            return Err(String::from("--ext produced empty extension set"));
        }

        Ok(ParseOutcome::Config(Self {
            root,
            cmd,
            shell,
            workdir,
            exts,
            debounce: Duration::from_millis(debounce_ms),
            reconcile_interval: Duration::from_secs(reconcile_sec.max(1)),
            log_level,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileState {
    Missing,
    Empty,
    NonEmpty { size: u64, hash64: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerReason {
    CreatedNonEmpty,
    BecameNonEmpty,
    ContentChanged,
    Deleted,
    BecameEmpty,
}

impl TriggerReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::CreatedNonEmpty => "CreatedNonEmpty",
            Self::BecameNonEmpty => "BecameNonEmpty",
            Self::ContentChanged => "ContentChanged",
            Self::Deleted => "Deleted",
            Self::BecameEmpty => "BecameEmpty",
        }
    }
}

#[derive(Debug, Clone)]
struct TriggerEvent {
    path: PathBuf,
    reason: TriggerReason,
}

#[derive(Debug, Clone)]
struct BuildDone {
    build_id: u64,
    success: bool,
    exit_code: Option<i32>,
    duration_ms: u128,
    error: Option<String>,
}

fn main() {
    match Config::parse() {
        Ok(ParseOutcome::Help) => {
            println!("{}", usage_text());
        }
        Ok(ParseOutcome::Config(cfg)) => {
            if let Err(err) = run(cfg) {
                eprintln!("fatal: {err}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    }
}

fn run(cfg: Config) -> Result<(), String> {
    log(
        cfg.log_level,
        LogLevel::Info,
        &format!(
            "starting mdwatch root={} workdir={} shell={} cmd={:?} ext={:?} debounce_ms={} reconcile_sec={}",
            cfg.root.display(),
            cfg.workdir.display(),
            cfg.shell.as_str(),
            cfg.cmd,
            cfg.exts,
            cfg.debounce.as_millis(),
            cfg.reconcile_interval.as_secs()
        ),
    );

    let mut states = scan_markdown_states(&cfg.root, &cfg.exts)
        .map_err(|e| format!("initial scan failed under {}: {e}", cfg.root.display()))?;
    log(
        cfg.log_level,
        LogLevel::Info,
        &format!("initial baseline loaded files={}", states.len()),
    );

    let (fs_tx, fs_rx) = mpsc::channel::<Result<Event, notify::Error>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = fs_tx.send(res);
    })
    .map_err(|e| format!("create watcher failed: {e}"))?;

    watcher
        .configure(NotifyConfig::default())
        .map_err(|e| format!("watcher configure failed: {e}"))?;
    watcher
        .watch(&cfg.root, RecursiveMode::Recursive)
        .map_err(|e| format!("watch root failed {}: {e}", cfg.root.display()))?;

    let (build_tx, build_rx) = mpsc::channel::<BuildDone>();

    let mut pending = false;
    let mut dirty = false;
    let mut build_running = false;
    let mut debounce_deadline: Option<Instant> = None;
    let mut next_reconcile = Instant::now() + cfg.reconcile_interval;
    let mut force_reconcile = false;
    let mut build_id: u64 = 0;

    loop {
        while let Ok(done) = build_rx.try_recv() {
            build_running = false;
            if let Some(err) = done.error {
                log(
                    cfg.log_level,
                    LogLevel::Error,
                    &format!(
                        "BUILD done id={} status=error duration_ms={} error={}",
                        done.build_id, done.duration_ms, err
                    ),
                );
            } else {
                let status = if done.success { "ok" } else { "fail" };
                log(
                    cfg.log_level,
                    LogLevel::Info,
                    &format!(
                        "BUILD done id={} status={} code={:?} duration_ms={}",
                        done.build_id, status, done.exit_code, done.duration_ms
                    ),
                );
            }

            if dirty {
                dirty = false;
                pending = true;
                debounce_deadline = Some(Instant::now());
                log(
                    cfg.log_level,
                    LogLevel::Info,
                    "new changes arrived during build; scheduling immediate follow-up build",
                );
            }
        }

        if force_reconcile || Instant::now() >= next_reconcile {
            let reconcile_start = Instant::now();
            match reconcile_root(&mut states, &cfg.root, &cfg.exts) {
                Ok(events) => {
                    if !events.is_empty() {
                        for event in &events {
                            log(
                                cfg.log_level,
                                LogLevel::Info,
                                &format!(
                                    "EVENT path={} decision={}",
                                    event.path.display(),
                                    event.reason.as_str()
                                ),
                            );
                        }
                        if build_running {
                            dirty = true;
                        } else {
                            pending = true;
                            debounce_deadline = Some(Instant::now() + cfg.debounce);
                        }
                    }
                    log(
                        cfg.log_level,
                        LogLevel::Info,
                        &format!(
                            "RECONCILE done changed_count={} duration_ms={}",
                            events.len(),
                            reconcile_start.elapsed().as_millis()
                        ),
                    );
                }
                Err(e) => {
                    log(
                        cfg.log_level,
                        LogLevel::Warn,
                        &format!("reconcile failed: {e}"),
                    );
                }
            }
            force_reconcile = false;
            next_reconcile = Instant::now() + cfg.reconcile_interval;
        }

        if pending
            && !build_running
            && debounce_deadline
                .map(|deadline| Instant::now() >= deadline)
                .unwrap_or(true)
        {
            pending = false;
            debounce_deadline = None;
            build_running = true;
            build_id += 1;
            log(
                cfg.log_level,
                LogLevel::Info,
                &format!(
                    "BUILD start id={} workdir={} cmd={:?}",
                    build_id,
                    cfg.workdir.display(),
                    cfg.cmd
                ),
            );
            spawn_build(
                build_id,
                cfg.cmd.clone(),
                cfg.shell,
                cfg.workdir.clone(),
                build_tx.clone(),
            );
        }

        let timeout = compute_poll_timeout(debounce_deadline, next_reconcile);
        match fs_rx.recv_timeout(timeout) {
            Ok(res) => {
                handle_notify_result(
                    res,
                    &cfg,
                    &mut states,
                    &mut pending,
                    &mut dirty,
                    build_running,
                    &mut debounce_deadline,
                    &mut force_reconcile,
                );
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(String::from("watcher event channel disconnected"));
            }
        }

        // Drain burst events so we do fewer build schedules in hot update streams.
        for _ in 0..2048 {
            match fs_rx.try_recv() {
                Ok(res) => {
                    handle_notify_result(
                        res,
                        &cfg,
                        &mut states,
                        &mut pending,
                        &mut dirty,
                        build_running,
                        &mut debounce_deadline,
                        &mut force_reconcile,
                    );
                }
                Err(_) => break,
            }
        }
    }
}

fn handle_notify_result(
    res: Result<Event, notify::Error>,
    cfg: &Config,
    states: &mut HashMap<PathBuf, FileState>,
    pending: &mut bool,
    dirty: &mut bool,
    build_running: bool,
    debounce_deadline: &mut Option<Instant>,
    force_reconcile: &mut bool,
) {
    let event = match res {
        Ok(ev) => ev,
        Err(err) => {
            log(
                cfg.log_level,
                LogLevel::Warn,
                &format!("notify error; forcing reconcile: {err}"),
            );
            *force_reconcile = true;
            return;
        }
    };

    if cfg.log_level >= LogLevel::Debug {
        log(
            cfg.log_level,
            LogLevel::Debug,
            &format!("notify event kind={:?} paths={:?}", event.kind, event.paths),
        );
    }

    if event.paths.is_empty() {
        *force_reconcile = true;
        return;
    }

    let mut triggered = false;
    for raw_path in event.paths {
        let path = if raw_path.is_absolute() {
            raw_path
        } else {
            cfg.root.join(raw_path)
        };
        if !path.starts_with(&cfg.root) {
            continue;
        }
        match process_path_event(states, &path, cfg) {
            Ok(events) => {
                if !events.is_empty() {
                    triggered = true;
                }
                for change in events {
                    log(
                        cfg.log_level,
                        LogLevel::Info,
                        &format!(
                            "EVENT path={} decision={}",
                            change.path.display(),
                            change.reason.as_str()
                        ),
                    );
                }
            }
            Err(e) => {
                log(
                    cfg.log_level,
                    LogLevel::Warn,
                    &format!("process event path={} failed: {e}", path.display()),
                );
                *force_reconcile = true;
            }
        }
    }

    if triggered {
        if build_running {
            *dirty = true;
        } else {
            *pending = true;
            *debounce_deadline = Some(Instant::now() + cfg.debounce);
        }
    }
}

fn process_path_event(
    states: &mut HashMap<PathBuf, FileState>,
    path: &Path,
    cfg: &Config,
) -> io::Result<Vec<TriggerEvent>> {
    if should_ignore_path(path) {
        return Ok(Vec::new());
    }

    let mut events = Vec::new();

    let is_markdown_or_known = is_markdown_path(path, &cfg.exts) || states.contains_key(path);
    if is_markdown_or_known {
        if let Some(reason) = apply_file_update(states, path)? {
            events.push(TriggerEvent {
                path: path.to_path_buf(),
                reason,
            });
        }
        return Ok(events);
    }

    if path.is_dir() {
        return reconcile_subtree(states, path, &cfg.exts);
    }

    // Handle deleted or renamed directories where notify gives only the parent path.
    let stale_paths: Vec<PathBuf> = states
        .keys()
        .filter(|p| p.starts_with(path))
        .cloned()
        .collect();
    if stale_paths.is_empty() {
        return Ok(events);
    }

    for stale in stale_paths {
        if let Some(old_state) = states.remove(&stale)
            && let Some(reason) = decide_transition(&old_state, &FileState::Missing)
        {
            events.push(TriggerEvent {
                path: stale,
                reason,
            });
        }
    }
    Ok(events)
}

fn apply_file_update(
    states: &mut HashMap<PathBuf, FileState>,
    path: &Path,
) -> io::Result<Option<TriggerReason>> {
    let old_state = states.get(path).cloned().unwrap_or(FileState::Missing);
    let new_state = snapshot_file_state(path)?;
    let decision = decide_transition(&old_state, &new_state);

    match new_state {
        FileState::Missing => {
            states.remove(path);
        }
        state => {
            states.insert(path.to_path_buf(), state);
        }
    }

    Ok(decision)
}

fn reconcile_subtree(
    states: &mut HashMap<PathBuf, FileState>,
    dir: &Path,
    exts: &HashSet<String>,
) -> io::Result<Vec<TriggerEvent>> {
    let mut seen = HashSet::<PathBuf>::new();
    let mut events = Vec::<TriggerEvent>::new();

    if dir.exists() && dir.is_dir() {
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.into_path();
            if !is_markdown_path(&path, exts) || should_ignore_path(&path) {
                continue;
            }
            let old_state = states.get(&path).cloned().unwrap_or(FileState::Missing);
            let new_state = snapshot_file_state(&path)?;
            if let Some(reason) = decide_transition(&old_state, &new_state) {
                events.push(TriggerEvent {
                    path: path.clone(),
                    reason,
                });
            }
            states.insert(path.clone(), new_state);
            seen.insert(path);
        }
    }

    let stale_paths: Vec<PathBuf> = states
        .keys()
        .filter(|path| path.starts_with(dir) && !seen.contains(*path))
        .cloned()
        .collect();

    for stale in stale_paths {
        if let Some(old_state) = states.remove(&stale)
            && let Some(reason) = decide_transition(&old_state, &FileState::Missing)
        {
            events.push(TriggerEvent {
                path: stale,
                reason,
            });
        }
    }

    Ok(events)
}

fn reconcile_root(
    states: &mut HashMap<PathBuf, FileState>,
    root: &Path,
    exts: &HashSet<String>,
) -> io::Result<Vec<TriggerEvent>> {
    reconcile_subtree(states, root, exts)
}

fn scan_markdown_states(
    root: &Path,
    exts: &HashSet<String>,
) -> io::Result<HashMap<PathBuf, FileState>> {
    let mut states = HashMap::<PathBuf, FileState>::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();
        if !is_markdown_path(&path, exts) || should_ignore_path(&path) {
            continue;
        }
        let state = snapshot_file_state(&path)?;
        if !matches!(state, FileState::Missing) {
            states.insert(path, state);
        }
    }
    Ok(states)
}

fn snapshot_file_state(path: &Path) -> io::Result<FileState> {
    let metadata = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(FileState::Missing),
        Err(err) => return Err(err),
    };

    if !metadata.is_file() {
        return Ok(FileState::Missing);
    }

    let size = metadata.len();
    if size == 0 {
        return Ok(FileState::Empty);
    }

    let hash64 = hash_file_xxh3(path)?;
    Ok(FileState::NonEmpty { size, hash64 })
}

fn hash_file_xxh3(path: &Path) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let mut hasher = Xxh3::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.digest())
}

fn decide_transition(old: &FileState, new: &FileState) -> Option<TriggerReason> {
    match (old, new) {
        (FileState::Missing, FileState::NonEmpty { .. }) => Some(TriggerReason::CreatedNonEmpty),
        (FileState::Empty, FileState::NonEmpty { .. }) => Some(TriggerReason::BecameNonEmpty),
        (
            FileState::NonEmpty {
                hash64: old_hash, ..
            },
            FileState::NonEmpty {
                hash64: new_hash, ..
            },
        ) if old_hash != new_hash => Some(TriggerReason::ContentChanged),
        (FileState::Empty, FileState::Missing)
        | (FileState::NonEmpty { .. }, FileState::Missing) => Some(TriggerReason::Deleted),
        (FileState::NonEmpty { .. }, FileState::Empty) => Some(TriggerReason::BecameEmpty),
        _ => None,
    }
}

fn parse_u64_arg(flag: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|e| format!("invalid value for {flag}: {value} ({e})"))
}

fn parse_extensions(csv: &str) -> Result<HashSet<String>, String> {
    let exts: HashSet<String> = csv
        .split(',')
        .map(|raw| raw.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|raw| !raw.is_empty())
        .collect();
    if exts.is_empty() {
        return Err(format!("invalid --ext value: {csv}"));
    }
    Ok(exts)
}

fn is_markdown_path(path: &Path, exts: &HashSet<String>) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    exts.contains(&ext.to_ascii_lowercase())
}

fn should_ignore_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };

    name.ends_with('~')
        || name.ends_with(".swp")
        || name.ends_with(".swo")
        || name.ends_with(".tmp")
        || name.starts_with(".#")
}

fn absolutize(base: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        base.join(target)
    }
}

fn spawn_build(
    build_id: u64,
    cmd: String,
    shell: ShellKind,
    workdir: PathBuf,
    tx: Sender<BuildDone>,
) {
    thread::spawn(move || {
        let start = Instant::now();
        let result = run_shell_command(shell, &cmd, &workdir);
        let elapsed = start.elapsed().as_millis();

        let done = match result {
            Ok(status) => BuildDone {
                build_id,
                success: status.success(),
                exit_code: status.code(),
                duration_ms: elapsed,
                error: None,
            },
            Err(err) => BuildDone {
                build_id,
                success: false,
                exit_code: None,
                duration_ms: elapsed,
                error: Some(format!("spawn build command failed: {err}")),
            },
        };

        let _ = tx.send(done);
    });
}

fn run_shell_command(
    shell: ShellKind,
    cmd: &str,
    workdir: &Path,
) -> io::Result<std::process::ExitStatus> {
    let mut command = match shell {
        ShellKind::Sh => {
            let mut c = Command::new("sh");
            c.arg("-c").arg(cmd);
            c
        }
        ShellKind::Bash => {
            let mut c = Command::new("bash");
            c.arg("-lc").arg(cmd);
            c
        }
        ShellKind::Cmd => {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(cmd);
            c
        }
        ShellKind::PowerShell => {
            let mut c = Command::new("powershell");
            c.arg("-NoProfile").arg("-Command").arg(cmd);
            c
        }
    };
    command.current_dir(workdir).status()
}

fn compute_poll_timeout(debounce_deadline: Option<Instant>, next_reconcile: Instant) -> Duration {
    let mut timeout = Duration::from_millis(500);
    if let Some(deadline) = debounce_deadline {
        let until = deadline.saturating_duration_since(Instant::now());
        if until < timeout {
            timeout = until;
        }
    }
    let until_reconcile = next_reconcile.saturating_duration_since(Instant::now());
    if until_reconcile < timeout {
        timeout = until_reconcile;
    }
    if timeout.is_zero() {
        Duration::from_millis(1)
    } else {
        timeout
    }
}

fn timestamp_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn log(current: LogLevel, level: LogLevel, message: &str) {
    if level <= current {
        eprintln!("[{}] {} {}", timestamp_unix_secs(), level.as_str(), message);
    }
}

fn usage_text() -> String {
    let default_cmd = default_cmd();
    let default_shell = default_shell().as_str();
    String::from(
        "Usage:
  mdwatch --root <path> [--workdir <path>] [--cmd <string>] [--shell <type>] [--ext <csv>] [--debounce-ms <n>] [--reconcile-sec <n>] [--log-level <level>]

Required:
  --root <path>            Markdown root directory to watch recursively

Optional:
  --workdir <path>         Working directory for command execution (default: current directory)
  --cmd <string>           Build command run through the selected shell
  --shell <type>           sh|bash|cmd|powershell
  --ext <csv>              Markdown extensions (default: md,markdown)
  --debounce-ms <n>        Build debounce window in milliseconds (default: 800)
  --reconcile-sec <n>      Full reconcile interval in seconds (default: 600)
  --log-level <level>      error|warn|info|debug (default: info)

Example:
  mdwatch --root /data/blog/markdown --workdir /srv/hugo/docker-compose --cmd \"./build.sh .env.runtime\" --shell sh
",
    )
    .replace("--cmd <string>           Build command run through the selected shell", &format!("--cmd <string>           Build command run through the selected shell (default: \"{}\")", default_cmd))
    .replace("--shell <type>           sh|bash|cmd|powershell", &format!("--shell <type>           sh|bash|cmd|powershell (default: {})", default_shell))
}

fn default_shell() -> ShellKind {
    if cfg!(windows) {
        ShellKind::Cmd
    } else {
        ShellKind::Sh
    }
}

fn default_cmd() -> String {
    if cfg!(windows) {
        String::from("docker compose --env-file .env.runtime run --rm --no-deps hugo-builder")
    } else {
        String::from("./build.sh .env.runtime")
    }
}

#[cfg(test)]
mod tests {
    use super::{FileState, TriggerReason, decide_transition};

    #[test]
    fn created_non_empty_triggers() {
        let old = FileState::Missing;
        let new = FileState::NonEmpty {
            size: 1,
            hash64: 42,
        };
        assert_eq!(
            decide_transition(&old, &new),
            Some(TriggerReason::CreatedNonEmpty)
        );
    }

    #[test]
    fn empty_to_non_empty_triggers() {
        let old = FileState::Empty;
        let new = FileState::NonEmpty { size: 2, hash64: 7 };
        assert_eq!(
            decide_transition(&old, &new),
            Some(TriggerReason::BecameNonEmpty)
        );
    }

    #[test]
    fn same_hash_non_empty_does_not_trigger() {
        let old = FileState::NonEmpty { size: 5, hash64: 9 };
        let new = FileState::NonEmpty { size: 5, hash64: 9 };
        assert_eq!(decide_transition(&old, &new), None);
    }

    #[test]
    fn hash_change_triggers() {
        let old = FileState::NonEmpty { size: 5, hash64: 9 };
        let new = FileState::NonEmpty {
            size: 5,
            hash64: 10,
        };
        assert_eq!(
            decide_transition(&old, &new),
            Some(TriggerReason::ContentChanged)
        );
    }

    #[test]
    fn delete_triggers() {
        let old = FileState::NonEmpty {
            size: 99,
            hash64: 777,
        };
        let new = FileState::Missing;
        assert_eq!(decide_transition(&old, &new), Some(TriggerReason::Deleted));
    }

    #[test]
    fn emptying_file_triggers() {
        let old = FileState::NonEmpty {
            size: 99,
            hash64: 777,
        };
        let new = FileState::Empty;
        assert_eq!(
            decide_transition(&old, &new),
            Some(TriggerReason::BecameEmpty)
        );
    }
}
