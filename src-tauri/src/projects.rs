use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// Suppresses the hide-on-blur behavior while a native dialog is open,
/// or while the user pinned the popover.
#[derive(Default)]
pub struct UiState {
    pub dialog_open: AtomicBool,
    pub window_pinned: AtomicBool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Preferred package manager: manual override > packageManager field
    /// > lockfile > default.
    pub package_manager: String,
    /// Whether the preferred package manager is actually installed locally.
    pub pm_installed: bool,
    /// Whether the package manager was manually chosen by the user.
    pub pm_overridden: bool,
    pub scripts: BTreeMap<String, String>,
    pub script_order: Vec<String>,
    #[serde(default)]
    pub autostart_scripts: Vec<String>,
    pub exists: bool,
    pub pinned: bool,
    pub git_branch: Option<String>,
    pub git_dirty: bool,
    /// Per-script launch configuration.
    pub launch: BTreeMap<String, LaunchConfig>,
    /// .env* files found in the project root (for the env_file picker).
    pub env_files: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LaunchConfig {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub env_file: Option<String>,
    #[serde(default)]
    pub extra_args: Option<String>,
}

impl LaunchConfig {
    pub fn is_empty(&self) -> bool {
        self.port.is_none()
            && self.env.is_empty()
            && self.env_file.is_none()
            && self.extra_args.as_deref().map_or(true, |s| s.trim().is_empty())
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct StoreEntry {
    path: String,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    package_manager: Option<String>,
    #[serde(default)]
    launch: BTreeMap<String, LaunchConfig>,
    #[serde(default)]
    script_order: Vec<String>,
    #[serde(default)]
    autostart_scripts: Vec<String>,
}

pub const KNOWN_PMS: [&str; 4] = ["pnpm", "yarn", "bun", "npm"];

/// Which package managers are actually executable on this machine.
/// Probed once with the login-shell PATH and cached.
pub fn installed_pms() -> &'static Vec<String> {
    static CACHE: OnceLock<Vec<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        KNOWN_PMS
            .iter()
            .filter(|pm| {
                #[cfg(windows)]
                let mut cmd = {
                    let mut c = Command::new("cmd");
                    c.args(["/C", &format!("{pm} --version")]);
                    c
                };
                #[cfg(not(windows))]
                let mut cmd = {
                    let mut c = Command::new(pm.to_string());
                    c.arg("--version");
                    c
                };
                cmd.env("PATH", crate::runner::build_path_env())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            })
            .map(|s| s.to_string())
            .collect()
    })
}

#[tauri::command]
pub fn installed_package_managers() -> Vec<String> {
    installed_pms().clone()
}

fn project_id(path: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn store_path<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("projects.json"))
}

fn load_entries<R: tauri::Runtime>(app: &AppHandle<R>) -> Vec<StoreEntry> {
    let Some(raw) = store_path(app).ok().and_then(|p| fs::read_to_string(p).ok()) else {
        return Vec::new();
    };
    // Current format: [{path, pinned}]. Legacy format: ["path", ...].
    serde_json::from_str::<Vec<StoreEntry>>(&raw)
        .or_else(|_| {
            serde_json::from_str::<Vec<String>>(&raw).map(|paths| {
                paths
                    .into_iter()
                    .map(|path| StoreEntry {
                        path,
                        pinned: false,
                        package_manager: None,
                        launch: BTreeMap::new(),
                        script_order: Vec::new(),
                        autostart_scripts: Vec::new(),
                    })
                    .collect()
            })
        })
        .unwrap_or_default()
}

fn save_entries<R: tauri::Runtime>(app: &AppHandle<R>, entries: &[StoreEntry]) -> Result<(), String> {
    let file = store_path(app)?;
    fs::write(file, serde_json::to_string_pretty(entries).unwrap()).map_err(|e| e.to_string())
}

fn lockfile_pm(dir: &Path) -> Option<&'static str> {
    if dir.join("pnpm-lock.yaml").exists() {
        Some("pnpm")
    } else if dir.join("yarn.lock").exists() {
        Some("yarn")
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        Some("bun")
    } else if dir.join("package-lock.json").exists() {
        Some("npm")
    } else {
        None
    }
}

/// "pnpm@9.1.0" (corepack packageManager field) -> "pnpm".
fn corepack_pm(pkg: Option<&serde_json::Value>) -> Option<String> {
    let field = pkg?.get("packageManager")?.as_str()?;
    let name = field.split('@').next()?.trim();
    KNOWN_PMS.contains(&name).then(|| name.to_string())
}

/// Branch name from .git/HEAD (no subprocess), dirty flag via git CLI.
fn git_info(dir: &Path) -> (Option<String>, bool) {
    let Ok(head) = fs::read_to_string(dir.join(".git").join("HEAD")) else {
        return (None, false);
    };
    let head = head.trim();
    let branch = head
        .strip_prefix("ref: refs/heads/")
        .map(|s| s.to_string())
        .unwrap_or_else(|| head.chars().take(7).collect());

    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(["status", "--porcelain", "-uno"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let dirty = cmd
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    (Some(branch), dirty)
}

fn build_project(entry: &StoreEntry) -> Project {
    let path = entry.path.as_str();
    let dir = Path::new(path);
    let pkg_file = dir.join("package.json");
    let mut name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let mut scripts = BTreeMap::new();
    let exists = pkg_file.exists();

    let pkg_json = fs::read_to_string(&pkg_file)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());

    if let Some(pkg) = &pkg_json {
        if let Some(n) = pkg.get("name").and_then(|v| v.as_str()) {
            if !n.is_empty() {
                name = n.to_string();
            }
        }
        if let Some(map) = pkg.get("scripts").and_then(|v| v.as_object()) {
            for (k, v) in map {
                if let Some(cmd) = v.as_str() {
                    scripts.insert(k.clone(), cmd.to_string());
                }
            }
        }
    }

    // Preference: manual override > corepack field > lockfile > default.
    let package_manager = entry
        .package_manager
        .clone()
        .or_else(|| corepack_pm(pkg_json.as_ref()))
        .or_else(|| lockfile_pm(dir).map(String::from))
        .unwrap_or_else(|| "pnpm".to_string());
    let pm_installed = installed_pms().iter().any(|p| p == &package_manager);

    let mut env_files: Vec<String> = fs::read_dir(dir)
        .ok()
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|n| n.starts_with(".env"))
                .collect()
        })
        .unwrap_or_default();
    env_files.sort();

    let (git_branch, git_dirty) = git_info(dir);
    let script_order: Vec<String> = entry
        .script_order
        .iter()
        .filter(|name| scripts.contains_key(*name))
        .cloned()
        .collect();
    let autostart_scripts: Vec<String> = entry
        .autostart_scripts
        .iter()
        .filter(|name| scripts.contains_key(*name))
        .cloned()
        .collect();

    Project {
        id: project_id(path),
        name,
        path: path.to_string(),
        package_manager,
        pm_installed,
        pm_overridden: entry.package_manager.is_some(),
        scripts,
        exists,
        pinned: entry.pinned,
        git_branch,
        git_dirty,
        launch: entry.launch.clone(),
        script_order,
        autostart_scripts,
        env_files,
    }
}

#[tauri::command]
pub fn list_projects<R: tauri::Runtime>(app: AppHandle<R>) -> Vec<Project> {
    let mut projects: Vec<Project> = load_entries(&app).iter().map(build_project).collect();
    // Pinned first; within each group keep the user-defined store order
    // (sort_by is stable).
    projects.sort_by(|a, b| b.pinned.cmp(&a.pinned));
    projects
}

/// Persist a manual ordering. `ids` is the full desired order; entries not
/// mentioned keep their relative order at the end.
#[tauri::command]
pub fn set_project_order<R: tauri::Runtime>(app: AppHandle<R>, ids: Vec<String>) -> Result<(), String> {
    let mut entries = load_entries(&app);
    entries.sort_by_key(|e| {
        let id = project_id(&e.path);
        ids.iter().position(|x| *x == id).unwrap_or(usize::MAX)
    });
    save_entries(&app, &entries)
}

pub fn add_project_path<R: tauri::Runtime>(app: &AppHandle<R>, path: &str) -> Result<Project, String> {
    if !Path::new(path).join("package.json").exists() {
        return Err("ERR_NO_PACKAGE_JSON".into());
    }
    let mut entries = load_entries(app);
    if !entries.iter().any(|e| e.path == path) {
        entries.push(StoreEntry {
            path: path.to_string(),
            pinned: false,
            package_manager: None,
            launch: BTreeMap::new(),
            script_order: Vec::new(),
            autostart_scripts: Vec::new(),
        });
        save_entries(app, &entries)?;
    }
    let entry = entries.into_iter().find(|e| e.path == path).unwrap();
    Ok(build_project(&entry))
}

#[tauri::command]
pub fn toggle_pin<R: tauri::Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let mut entries = load_entries(&app);
    for e in entries.iter_mut() {
        if project_id(&e.path) == id {
            e.pinned = !e.pinned;
        }
    }
    save_entries(&app, &entries)
}

/// Persist per-project script order. Unknown script names are ignored on read,
/// so keeping only the submitted order is enough.
#[tauri::command]
pub fn set_script_order<R: tauri::Runtime>(
    app: AppHandle<R>,
    id: String,
    scripts: Vec<String>,
) -> Result<(), String> {
    let mut entries = load_entries(&app);
    for e in entries.iter_mut() {
        if project_id(&e.path) == id {
            e.script_order = scripts.clone();
        }
    }
    save_entries(&app, &entries)
}

#[tauri::command]
pub fn set_script_autostart<R: tauri::Runtime>(
    app: AppHandle<R>,
    id: String,
    script: String,
    enabled: bool,
) -> Result<(), String> {
    let mut entries = load_entries(&app);
    for e in entries.iter_mut() {
        if project_id(&e.path) == id {
            if enabled {
                if !e.autostart_scripts.iter().any(|s| s == &script) {
                    e.autostart_scripts.push(script.clone());
                }
            } else {
                e.autostart_scripts.retain(|s| s != &script);
            }
        }
    }
    save_entries(&app, &entries)
}

/// Manually pin a project to a package manager; `None` restores auto-detect.
#[tauri::command]
pub fn set_package_manager<R: tauri::Runtime>(
    app: AppHandle<R>,
    id: String,
    pm: Option<String>,
) -> Result<(), String> {
    if let Some(pm) = &pm {
        if !KNOWN_PMS.contains(&pm.as_str()) {
            return Err(format!("unknown package manager: {pm}"));
        }
    }
    let mut entries = load_entries(&app);
    for e in entries.iter_mut() {
        if project_id(&e.path) == id {
            e.package_manager = pm.clone();
        }
    }
    save_entries(&app, &entries)
}

/// Per-script launch config; `None` clears it.
#[tauri::command]
pub fn set_launch_config<R: tauri::Runtime>(
    app: AppHandle<R>,
    id: String,
    script: String,
    config: Option<LaunchConfig>,
) -> Result<(), String> {
    let mut entries = load_entries(&app);
    for e in entries.iter_mut() {
        if project_id(&e.path) == id {
            match &config {
                Some(cfg) if !cfg.is_empty() => {
                    e.launch.insert(script.clone(), cfg.clone());
                }
                _ => {
                    e.launch.remove(&script);
                }
            }
        }
    }
    save_entries(&app, &entries)
}

pub fn launch_config_for<R: tauri::Runtime>(
    app: &AppHandle<R>,
    id: &str,
    script: &str,
) -> Option<LaunchConfig> {
    load_entries(app)
        .into_iter()
        .find(|e| project_id(&e.path) == id)
        .and_then(|e| e.launch.get(script).cloned())
}

#[tauri::command]
pub async fn pick_and_add_project<R: tauri::Runtime>(
    app: AppHandle<R>,
    ui: State<'_, UiState>,
) -> Result<Option<Project>, String> {
    ui.dialog_open.store(true, Ordering::SeqCst);
    let picked = app.dialog().file().blocking_pick_folder();
    ui.dialog_open.store(false, Ordering::SeqCst);

    // Bring the popover back after the native dialog stole focus.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }

    let Some(folder) = picked else {
        return Ok(None);
    };
    let path = folder
        .into_path()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();

    add_project_path(&app, &path).map(Some)
}

#[tauri::command]
pub fn remove_project<R: tauri::Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let entries: Vec<StoreEntry> = load_entries(&app)
        .into_iter()
        .filter(|e| project_id(&e.path) != id)
        .collect();
    save_entries(&app, &entries)
}

#[cfg(target_os = "macos")]
fn try_open_with(app_name: &str, path: &str) -> bool {
    Command::new("open")
        .args(["-a", app_name, path])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[tauri::command]
pub fn open_in_editor(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        for editor in ["Cursor", "Visual Studio Code", "Zed", "WebStorm"] {
            if try_open_with(editor, &path) {
                return Ok(());
            }
        }
        Err("ERR_NO_EDITOR".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        // `code`/`cursor` CLI shims work on both Windows (via cmd) and Linux.
        for editor in ["cursor", "code"] {
            #[cfg(windows)]
            let ok = Command::new("cmd")
                .args(["/C", editor, &path])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            #[cfg(not(windows))]
            let ok = Command::new(editor)
                .arg(&path)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                return Ok(());
            }
        }
        Err("ERR_NO_EDITOR".into())
    }
}

#[tauri::command]
pub fn open_in_terminal(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        for term in ["iTerm", "Warp", "Terminal"] {
            if try_open_with(term, &path) {
                return Ok(());
            }
        }
        Err("ERR_NO_TERMINAL".into())
    }
    #[cfg(target_os = "windows")]
    {
        // Prefer Windows Terminal, fall back to a plain cmd window.
        let wt = Command::new("cmd")
            .args(["/C", "wt", "-d", &path])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if wt {
            return Ok(());
        }
        Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", &format!("cd /d {path}")])
            .status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for term in ["x-terminal-emulator", "gnome-terminal", "konsole"] {
            if Command::new(term)
                .current_dir(&path)
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }
        Err("ERR_NO_TERMINAL".into())
    }
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";

    Command::new(cmd)
        .arg(&path)
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}
