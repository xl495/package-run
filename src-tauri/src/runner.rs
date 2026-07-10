use serde::Serialize;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
};
use tauri::{AppHandle, Emitter, Manager, State};

/// One entry per running script, keyed by "{project_id}:{script}".
pub struct RunnerState {
    pub tasks: Arc<Mutex<HashMap<String, TaskHandle>>>,
}

impl Default for RunnerState {
    fn default() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub struct TaskHandle {
    pub child: Child,
    pub url: Option<String>,
    pub log_file: PathBuf,
}

#[derive(Serialize, Clone, Debug)]
pub struct TaskInfo {
    pub key: String,
    pub pid: u32,
    pub url: Option<String>,
    pub log_file: String,
}

#[derive(Serialize, Clone)]
struct LogPayload {
    key: String,
    line: String,
}

#[derive(Serialize, Clone)]
struct ExitPayload {
    key: String,
    code: Option<i32>,
}

#[derive(Serialize, Clone)]
struct UrlPayload {
    key: String,
    url: String,
}

/// GUI apps launched from Finder/menu bar don't inherit the terminal PATH,
/// so tools installed via nvm/volta/homebrew (node!) are invisible. Query the
/// user's login shell once and cache the result. On Windows GUI apps inherit
/// the user PATH from the registry, so nothing to do there.
#[cfg(unix)]
fn user_shell_path() -> &'static str {
    static USER_PATH: OnceLock<String> = OnceLock::new();
    USER_PATH.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let out = Command::new(&shell)
            .args(["-ilc", "echo -n \"$PATH\""])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout);
                // rc files may print noise; the PATH is on the last line.
                let last = s.lines().last().unwrap_or("").trim().to_string();
                if last.contains('/') {
                    last
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    })
}

#[cfg(not(unix))]
fn user_shell_path() -> &'static str {
    ""
}

pub fn build_path_env() -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let home = std::env::var("HOME").unwrap_or_default();
    let fallbacks = if cfg!(unix) {
        format!(
            "/usr/local/bin:/opt/homebrew/bin:{home}/.local/share/pnpm:{home}/Library/pnpm:{home}/.volta/bin"
        )
    } else {
        String::new()
    };
    [
        user_shell_path().to_string(),
        std::env::var("PATH").unwrap_or_default(),
        fallbacks,
    ]
    .iter()
    .filter(|s| !s.is_empty())
    .cloned()
    .collect::<Vec<_>>()
    .join(sep)
}

fn extract_url(line: &str) -> Option<String> {
    // Match things like http://localhost:5173/ printed by vite/next/webpack.
    let start = line.find("http://").or_else(|| line.find("https://"))?;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '\u{1b}' || c == '"' || c == '\'')
        .unwrap_or(rest.len());
    let url = rest[..end].trim_end_matches('/').to_string();
    (url.contains("localhost") || url.contains("127.0.0.1") || url.contains("0.0.0.0"))
        .then_some(url)
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip CSI/OSC escape sequences.
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Minimal dotenv parser: KEY=VALUE lines, `export` prefixes and quotes
/// stripped, comments and blank lines ignored.
fn parse_env_file(path: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    let mut vars = std::collections::BTreeMap::new();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return vars;
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            vars.insert(k.trim().to_string(), v.to_string());
        }
    }
    vars
}

fn now_str() -> String {
    // Good enough for log headers without pulling in chrono.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

fn append_log(file: &Arc<Mutex<File>>, line: &str) {
    if let Ok(mut f) = file.lock() {
        let _ = writeln!(f, "{line}");
    }
}

fn pipe_reader<R: tauri::Runtime, T: std::io::Read + Send + 'static>(
    app: AppHandle<R>,
    key: String,
    reader: T,
    tasks: Arc<Mutex<HashMap<String, TaskHandle>>>,
    log: Arc<Mutex<File>>,
) {
    std::thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines().map_while(Result::ok) {
            let clean = strip_ansi(&line);
            append_log(&log, &clean);
            if let Some(url) = extract_url(&clean) {
                let mut guard = tasks.lock().unwrap();
                if let Some(task) = guard.get_mut(&key) {
                    if task.url.is_none() {
                        task.url = Some(url.clone());
                        let _ = app.emit("task-url", UrlPayload { key: key.clone(), url });
                    }
                }
            }
            let _ = app.emit(
                "task-log",
                LogPayload {
                    key: key.clone(),
                    line: clean,
                },
            );
        }
    });
}

fn log_file_path<R: tauri::Runtime>(app: &AppHandle<R>, project_id: &str, script: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("logs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe_script: String = script
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    Ok(dir.join(format!("{project_id}-{safe_script}.log")))
}

#[tauri::command]
pub fn start_script<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, RunnerState>,
    project_id: String,
    project_path: String,
    package_manager: String,
    script: String,
) -> Result<TaskInfo, String> {
    let key = format!("{project_id}:{script}");

    // Hold the lock through spawn+insert so two rapid clicks can't both pass
    // the duplicate check and start the same script twice.
    let mut tasks = state.tasks.lock().unwrap();
    if tasks.contains_key(&key) {
        return Err("ERR_ALREADY_RUNNING".into());
    }

    // The repo may prefer a package manager that isn't installed here
    // (e.g. fresh clone with pnpm-lock.yaml but no pnpm locally).
    if !crate::projects::installed_pms().iter().any(|p| p == &package_manager) {
        return Err(format!("ERR_PM_MISSING|{package_manager}"));
    }

    let log_path = log_file_path(&app, &project_id, &script)?;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("ERR_LOG_FILE|{e}"))?;
    let log = Arc::new(Mutex::new(log_file));
    append_log(
        &log,
        &format!("\n===== {package_manager} run {script} @ {} ({}) =====", now_str(), project_path),
    );

    // Apply the per-script launch config (env / env_file / port / args).
    let launch = crate::projects::launch_config_for(&app, &project_id, &script);
    let mut extra_env: Vec<(String, String)> = Vec::new();
    let mut extra_args: Vec<String> = Vec::new();
    if let Some(cfg) = &launch {
        if let Some(file) = &cfg.env_file {
            let full = std::path::Path::new(&project_path).join(file);
            extra_env.extend(parse_env_file(&full));
        }
        // Explicit env entries override the env_file.
        extra_env.extend(cfg.env.iter().map(|(k, v)| (k.clone(), v.clone())));
        if let Some(port) = cfg.port {
            // Cover both conventions: PORT env (CRA/Next/node servers) and
            // --port flag (vite and friends).
            extra_env.push(("PORT".into(), port.to_string()));
            extra_args.push("--port".into());
            extra_args.push(port.to_string());
        }
        if let Some(args) = &cfg.extra_args {
            extra_args.extend(args.split_whitespace().map(String::from));
        }
    }

    // On Windows, pnpm/npm/yarn are .cmd shims that must go through cmd.exe.
    #[cfg(windows)]
    let mut cmd = {
        let mut parts = vec![package_manager.clone(), "run".into(), script.clone()];
        if !extra_args.is_empty() {
            // npm forwards args to the script only after `--`.
            if package_manager == "npm" {
                parts.push("--".into());
            }
            parts.extend(extra_args.iter().cloned());
        }
        let mut c = Command::new("cmd");
        c.args(["/C", &parts.join(" ")]);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new(&package_manager);
        c.arg("run").arg(&script);
        if !extra_args.is_empty() {
            if package_manager == "npm" {
                c.arg("--");
            }
            c.args(&extra_args);
        }
        c
    };

    cmd.current_dir(&project_path)
        .env("FORCE_COLOR", "0")
        .env("PATH", build_path_env())
        .envs(extra_env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Put the child in its own process group so we can kill the whole tree
    // (pnpm spawns node which spawns vite, etc).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    // Don't flash a console window for every script on Windows.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("ERR_SPAWN|{package_manager}|{e}"))?;
    let pid = child.id();

    if let Some(stdout) = child.stdout.take() {
        pipe_reader(app.clone(), key.clone(), stdout, state.tasks.clone(), log.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        pipe_reader(app.clone(), key.clone(), stderr, state.tasks.clone(), log.clone());
    }

    tasks.insert(
        key.clone(),
        TaskHandle {
            child,
            url: None,
            log_file: log_path.clone(),
        },
    );
    drop(tasks);

    // Watcher thread: emit an event when the process exits on its own.
    {
        let tasks = state.tasks.clone();
        let app = app.clone();
        let key = key.clone();
        let log = log.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let mut guard = tasks.lock().unwrap();
            match guard.get_mut(&key) {
                Some(task) => match task.child.try_wait() {
                    Ok(Some(status)) => {
                        guard.remove(&key);
                        drop(guard);
                        append_log(&log, &format!("===== process exited with code {:?} =====", status.code()));
                        let _ = app.emit(
                            "task-exit",
                            ExitPayload {
                                key: key.clone(),
                                code: status.code(),
                            },
                        );
                        break;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        guard.remove(&key);
                        drop(guard);
                        let _ = app.emit("task-exit", ExitPayload { key: key.clone(), code: None });
                        break;
                    }
                },
                // stop_script already removed it and emitted the event.
                None => break,
            }
        });
    }

    Ok(TaskInfo {
        key,
        pid,
        url: None,
        log_file: log_path.to_string_lossy().to_string(),
    })
}

fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        unsafe {
            // Negative pid == the whole process group.
            libc::kill(-pid, libc::SIGTERM);
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
        if child.try_wait().ok().flatten().is_none() {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
    #[cfg(windows)]
    {
        // /T kills the whole child tree (cmd -> pnpm -> node -> vite).
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &child.id().to_string()])
            .output();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[tauri::command]
pub fn stop_script<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, RunnerState>,
    project_id: String,
    script: String,
) -> Result<(), String> {
    let key = format!("{project_id}:{script}");
    let task = state.tasks.lock().unwrap().remove(&key);
    match task {
        Some(mut task) => {
            kill_tree(&mut task.child);
            if let Ok(mut f) = OpenOptions::new().append(true).open(&task.log_file) {
                let _ = writeln!(f, "===== stopped by user @ {} =====", now_str());
            }
            let _ = app.emit("task-exit", ExitPayload { key, code: None });
            Ok(())
        }
        None => Err("ERR_NOT_RUNNING".into()),
    }
}

#[tauri::command]
pub fn running_tasks(state: State<'_, RunnerState>) -> Vec<TaskInfo> {
    state
        .tasks
        .lock()
        .unwrap()
        .iter()
        .map(|(k, t)| TaskInfo {
            key: k.clone(),
            pid: t.child.id(),
            url: t.url.clone(),
            log_file: t.log_file.to_string_lossy().to_string(),
        })
        .collect()
}

#[tauri::command]
pub fn get_log_file<R: tauri::Runtime>(app: AppHandle<R>, project_id: String, script: String) -> Result<String, String> {
    let path = log_file_path(&app, &project_id, &script)?;
    if !path.exists() {
        return Err("ERR_NO_LOG_FILE".into());
    }
    Ok(path.to_string_lossy().to_string())
}

/// Kill everything on app exit so no orphaned dev servers stay behind.
pub fn shutdown_all(state: &RunnerState) {
    let mut tasks = state.tasks.lock().unwrap();
    for (_, task) in tasks.iter_mut() {
        kill_tree(&mut task.child);
    }
    tasks.clear();
}
