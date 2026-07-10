import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  enable as enableAutostart,
  disable as disableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import {
  ArrowUpCircle,
  ChevronRight,
  ChevronsUpDown,
  ExternalLink,
  FolderOpen,
  Globe2,
  Keyboard,
  MonitorUp,
  Moon,
  Pin,
  PinOff,
  Play,
  Plus,
  Power,
  RotateCw,
  Search,
  Settings,
  Square,
  Star,
  Sun,
  TerminalSquare,
  Trash2,
  X,
} from "lucide-react";
import { detectLang, persistLang, STRINGS, translateError, type Lang } from "./i18n";
import {
  checkForUpdate,
  dismissUpdate,
  RELEASES_PAGE,
  type AvailableUpdate,
} from "./update";
import "./App.css";

interface LaunchConfig {
  port: number | null;
  env: Record<string, string>;
  env_file: string | null;
  extra_args: string | null;
}

interface Project {
  id: string;
  name: string;
  path: string;
  package_manager: string;
  pm_installed: boolean;
  pm_overridden: boolean;
  scripts: Record<string, string>;
  exists: boolean;
  pinned: boolean;
  git_branch: string | null;
  git_dirty: boolean;
  launch: Record<string, LaunchConfig>;
  autostart_scripts: string[];
  env_files: string[];
}

interface PortInfo {
  port: number;
  pid: number;
  name: string;
}

interface TaskInfo {
  key: string;
  pid: number;
  url: string | null;
  log_file: string;
}

// macOS gets the menu-bar popover look; other platforms are a normal window.
const IS_MAC = navigator.userAgent.includes("Mac");

const PREFERRED_ORDER = ["dev", "start", "serve", "build", "preview", "test", "lint"];

type Theme = "system" | "light" | "dark";

const THEME_KEY = "package-run-theme";

function detectTheme(): Theme {
  const saved = localStorage.getItem(THEME_KEY);
  return saved === "light" || saved === "dark" || saved === "system" ? saved : "system";
}

function persistTheme(theme: Theme) {
  localStorage.setItem(THEME_KEY, theme);
}

function nextTheme(theme: Theme): Theme {
  if (theme === "system") return "dark";
  if (theme === "dark") return "light";
  return "system";
}

/** KeyboardEvent.code -> token accepted by the global-shortcut plugin. */
function codeToKey(code: string): string | null {
  if (/^Key([A-Z])$/.test(code)) return code.slice(3);
  if (/^Digit(\d)$/.test(code)) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  const map: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Backquote: "`",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
  };
  return map[code] ?? null;
}

function prettyShortcut(sc: string): string {
  return sc
    .split("+")
    .map((part) => {
      const p = part.toLowerCase();
      if (p === "super") return "⌘";
      if (p === "ctrl") return "⌃";
      if (p === "alt") return "⌥";
      if (p === "shift") return "⇧";
      if (p === "space") return "Space";
      return part.toUpperCase();
    })
    .join("");
}



function renderLogLine(line: string) {
  if (!line) return " ";
  const urlRe = /(https?:\/\/[^\s"'<>\)]+)/g;
  const parts: ReactNode[] = [];
  let last = 0;
  for (const match of line.matchAll(urlRe)) {
    const url = match[0];
    const index = match.index ?? 0;
    if (index > last) parts.push(line.slice(last, index));
    parts.push(
      <button
        key={`${index}-${url}`}
        className="log-link"
        title={url}
        onClick={() => openUrl(url)}
      >
        {url}
      </button>,
    );
    last = index + url.length;
  }
  if (last < line.length) parts.push(line.slice(last));
  return parts.length ? parts : line;
}

function orderedScripts(project: Project, running: Map<string, TaskInfo>): string[] {
  return sortScripts(project.scripts).sort((a, b) => {
    const ar = running.has(`${project.id}:${a}`);
    const br = running.has(`${project.id}:${b}`);
    if (ar !== br) return ar ? -1 : 1;
    return 0;
  });
}

function sortScripts(scripts: Record<string, string>): string[] {
  return Object.keys(scripts).sort((a, b) => {
    const ia = PREFERRED_ORDER.indexOf(a);
    const ib = PREFERRED_ORDER.indexOf(b);
    if (ia !== -1 && ib !== -1) return ia - ib;
    if (ia !== -1) return -1;
    if (ib !== -1) return 1;
    return a.localeCompare(b);
  });
}

export default function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [running, setRunning] = useState<Map<string, TaskInfo>>(new Map());
  const [stopping, setStopping] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<string | null>(null);
  const [logKey, setLogKey] = useState<string | null>(null);
  const [logs, setLogs] = useState<Map<string, string[]>>(new Map());
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [autoStart, setAutoStart] = useState(false);
  const [winPinned, setWinPinned] = useState(false);
  const [portQuery, setPortQuery] = useState("");
  const [portResult, setPortResult] = useState<PortInfo | null | "free">(null);
  const [shortcut, setShortcut] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [autostartBooted, setAutostartBooted] = useState(false);
  const [lang, setLang] = useState<Lang>(detectLang);
  const [theme, setTheme] = useState<Theme>(detectTheme);
  const [installedPms, setInstalledPms] = useState<string[]>([]);
  const [pmMenuFor, setPmMenuFor] = useState<string | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  // Launch-config editor: which "projectId:script" is open + form fields.
  const [cfgFor, setCfgFor] = useState<string | null>(null);
  const [cfgPort, setCfgPort] = useState("");
  const [cfgEnv, setCfgEnv] = useState("");
  const [cfgEnvFile, setCfgEnvFile] = useState("");
  const [cfgArgs, setCfgArgs] = useState("");
  const [appVersion, setAppVersion] = useState("");
  const [updateInfo, setUpdateInfo] = useState<AvailableUpdate | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);
  const t = STRINGS[lang];
  const totalRunning = Math.max(0, running.size - stopping.size);
  // Guards against double-clicking ▶ before the backend responds.
  const busyRef = useRef<Set<string>>(new Set());

  const refresh = useCallback(async () => {
    const [list, tasks] = await Promise.all([
      invoke<Project[]>("list_projects"),
      invoke<TaskInfo[]>("running_tasks"),
    ]);
    setProjects(list);
    setRunning((prev) => {
      const next = new Map(tasks.map((t) => [t.key, t]));
      for (const key of stopping) {
        if (prev.has(key)) next.delete(key);
      }
      return next;
    });
  }, [stopping]);

  const markStopping = useCallback((key: string) => {
    setStopping((prev) => new Set(prev).add(key));
  }, []);

  const clearStopping = useCallback((key: string) => {
    setStopping((prev) => {
      if (!prev.has(key)) return prev;
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  }, []);

  const removeRunningTask = useCallback((key: string) => {
    setRunning((prev) => {
      if (!prev.has(key)) return prev;
      const next = new Map(prev);
      next.delete(key);
      return next;
    });
    clearStopping(key);
  }, [clearStopping]);

  const flash = useCallback((msg: string) => {
    setError(msg);
    window.setTimeout(() => setError(null), 4000);
  }, []);

  const runUpdateCheck = useCallback(
    async (opts?: { ignoreDismissed?: boolean; manual?: boolean }) => {
      setUpdateChecking(true);
      try {
        const version = appVersion || (await getVersion());
        if (!appVersion) setAppVersion(version);
        const found = await checkForUpdate(version, {
          ignoreDismissed: opts?.ignoreDismissed,
        });
        setUpdateInfo(found);
        if (opts?.manual) {
          if (found) {
            flash(t.updateAvailable(found.version));
          } else {
            flash(t.updateLatest(version));
          }
        }
      } catch {
        if (opts?.manual) flash(t.updateFailed);
      } finally {
        setUpdateChecking(false);
      }
    },
    [appVersion, flash, t],
  );

  useEffect(() => {
    refresh();
    isAutostartEnabled().then(setAutoStart).catch(() => {});
    invoke<string | null>("get_shortcut").then(setShortcut).catch(() => {});
    invoke<string[]>("installed_package_managers").then(setInstalledPms).catch(() => {});
    getVersion()
      .then((v) => {
        setAppVersion(v);
        // Soft check shortly after launch so UI paints first.
        window.setTimeout(() => {
          checkForUpdate(v)
            .then((found) => setUpdateInfo(found))
            .catch(() => {});
        }, 2500);
      })
      .catch(() => {});
    const unlisteners = [
      listen<{ key: string; line: string }>("task-log", (e) => {
        setLogs((prev) => {
          const next = new Map(prev);
          const lines = [...(next.get(e.payload.key) ?? []), e.payload.line];
          next.set(e.payload.key, lines.slice(-500));
          return next;
        });
        // Port conflict? Surface the port checker with the culprit pre-filled.
        const m = e.payload.line.match(
          /(?:EADDRINUSE|address already in use|is already in use|Port (\d+) is in use).*?(?::(\d+))?/i,
        );
        if (m) {
          const port = m[1] ?? m[2] ?? e.payload.line.match(/:(\d{2,5})/)?.[1];
          if (port) {
            setPortQuery(port);
            queryPort(port);
            flash(t.portConflictDetected(port));
          }
        }
      }),
      listen<{ key: string; code: number | null }>("task-exit", (e) => {
        removeRunningTask(e.payload.key);
        if (e.payload.code != null && e.payload.code !== 0) {
          const script = e.payload.key.split(":").slice(1).join(":");
          flash(t.scriptCrashed(script, e.payload.code));
        }
      }),
      listen<{ key: string; url: string }>("task-url", (e) => {
        setRunning((prev) => {
          const next = new Map(prev);
          const t = next.get(e.payload.key);
          if (t) next.set(e.payload.key, { ...t, url: e.payload.url });
          return next;
        });
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((fn) => fn()));
    };
  }, [refresh, removeRunningTask, t]);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [logs, logKey]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    invoke("set_running_badge", { count: totalRunning }).catch(() => {});
  }, [totalRunning]);

  const flashErr = (e: unknown) => flash(translateError(String(e), t));

  const toggleLang = () => {
    const next: Lang = lang === "zh" ? "en" : "zh";
    setLang(next);
    persistLang(next);
  };

  const themeLabel = (value: Theme = theme) => {
    if (value === "dark") return t.themeDark;
    if (value === "light") return t.themeLight;
    return t.themeSystem;
  };

  const toggleTheme = () => {
    const next = nextTheme(theme);
    setTheme(next);
    persistTheme(next);
  };

  const addProject = async () => {
    try {
      const p = await invoke<Project | null>("pick_and_add_project");
      if (p) await refresh();
    } catch (e) {
      flashErr(e);
    }
  };

  const removeProject = async (id: string) => {
    await invoke("remove_project", { id });
    await refresh();
  };

  const startScript = async (p: Project, script: string) => {
    const busyKey = `${p.id}:${script}`;
    if (busyRef.current.has(busyKey)) return;
    busyRef.current.add(busyKey);
    try {
      clearStopping(busyKey);
      const info = await invoke<TaskInfo>("start_script", {
        projectId: p.id,
        projectPath: p.path,
        packageManager: p.package_manager,
        script,
      });
      setRunning((prev) => new Map(prev).set(info.key, info));
      setLogs((prev) => {
        const next = new Map(prev);
        next.set(info.key, []);
        return next;
      });
      setLogKey(info.key);
    } catch (e) {
      flashErr(e);
    } finally {
      busyRef.current.delete(busyKey);
    }
  };

  const stopScript = async (p: Project, script: string) => {
    const key = `${p.id}:${script}`;
    if (stopping.has(key)) return;
    markStopping(key);
    removeRunningTask(key);
    if (logKey === key) setLogKey(null);
    try {
      await invoke("stop_script", { projectId: p.id, script });
      window.setTimeout(() => refresh().catch(() => {}), 250);
    } catch (e) {
      clearStopping(key);
      await refresh().catch(() => {});
      flashErr(e);
    }
  };

  const restartScript = async (p: Project, script: string) => {
    const busyKey = `${p.id}:${script}`;
    if (busyRef.current.has(busyKey)) return;
    try {
      markStopping(busyKey);
      removeRunningTask(busyKey);
      await invoke("stop_script", { projectId: p.id, script });
      // Give the stop event a moment to flush before the new start.
      await new Promise((r) => setTimeout(r, 150));
    } catch {
      // Not running anymore — just start it.
    }
    await startScript(p, script);
  };

  const togglePin = async (id: string) => {
    await invoke("toggle_pin", { id });
    await refresh();
  };

  // Drag-and-drop priority ordering (disabled while searching, because the
  // filtered view doesn't represent the true order).
  const dropOn = async (targetId: string) => {
    if (!dragId || dragId === targetId) {
      setDragId(null);
      setDragOverId(null);
      return;
    }
    const ids = projects.map((p) => p.id);
    const from = ids.indexOf(dragId);
    const to = ids.indexOf(targetId);
    if (from === -1 || to === -1) return;
    ids.splice(from, 1);
    ids.splice(to, 0, dragId);
    setDragId(null);
    setDragOverId(null);
    // Optimistic reorder so the UI doesn't flicker.
    setProjects((prev) => {
      const byId = new Map(prev.map((p) => [p.id, p]));
      return ids.map((id) => byId.get(id)!).filter(Boolean);
    });
    await invoke("set_project_order", { ids });
    await refresh();
  };

  const choosePm = async (id: string, pm: string | null) => {
    try {
      await invoke("set_package_manager", { id, pm });
      setPmMenuFor(null);
      await refresh();
    } catch (e) {
      flashErr(e);
    }
  };

  const openCfgEditor = (p: Project, script: string) => {
    const key = `${p.id}:${script}`;
    if (cfgFor === key) {
      setCfgFor(null);
      return;
    }
    const cfg = p.launch[script];
    setCfgPort(cfg?.port ? String(cfg.port) : "");
    setCfgEnv(
      cfg ? Object.entries(cfg.env).map(([k, v]) => `${k}=${v}`).join("\n") : "",
    );
    setCfgEnvFile(cfg?.env_file ?? "");
    setCfgArgs(cfg?.extra_args ?? "");
    setCfgFor(key);
  };

  const saveCfg = async (p: Project, script: string) => {
    const env: Record<string, string> = {};
    for (const line of cfgEnv.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      const eq = trimmed.indexOf("=");
      if (eq <= 0) {
        flash(t.cfgInvalidEnv(trimmed));
        return;
      }
      env[trimmed.slice(0, eq).trim()] = trimmed.slice(eq + 1).trim();
    }
    const port = cfgPort ? parseInt(cfgPort, 10) : null;
    if (cfgPort && (!port || port < 1 || port > 65535)) {
      flash(t.invalidPort);
      return;
    }
    const config: LaunchConfig = {
      port,
      env,
      env_file: cfgEnvFile || null,
      extra_args: cfgArgs.trim() || null,
    };
    const isEmpty = !port && !Object.keys(env).length && !config.env_file && !config.extra_args;
    try {
      await invoke("set_launch_config", {
        id: p.id,
        script,
        config: isEmpty ? null : config,
      });
      setCfgFor(null);
      flash(isEmpty ? t.cfgCleared : t.cfgSaved);
      await refresh();
    } catch (e) {
      flashErr(e);
    }
  };

  const clearCfg = async (p: Project, script: string) => {
    try {
      await invoke("set_launch_config", { id: p.id, script, config: null });
      setCfgFor(null);
      flash(t.cfgCleared);
      await refresh();
    } catch (e) {
      flashErr(e);
    }
  };

  const toggleScriptAutostart = async (p: Project, script: string) => {
    const enabled = !p.autostart_scripts.includes(script);
    try {
      await invoke("set_script_autostart", { id: p.id, script, enabled });
      await refresh();
    } catch (e) {
      flashErr(e);
    }
  };

  const toggleAutostart = async () => {
    try {
      if (autoStart) {
        await disableAutostart();
        setAutoStart(false);
      } else {
        await enableAutostart();
        setAutoStart(true);
      }
    } catch (e) {
      flashErr(e);
    }
  };

  const toggleWinPin = async () => {
    const next = !winPinned;
    setWinPinned(next);
    await invoke("set_window_pinned", { pinned: next });
  };

  const queryPort = async (raw: string) => {
    const port = parseInt(raw, 10);
    if (!port || port < 1 || port > 65535) {
      flash(t.invalidPort);
      return;
    }
    try {
      const info = await invoke<PortInfo | null>("port_info", { port });
      setPortResult(info ?? "free");
    } catch (e) {
      flashErr(e);
    }
  };

  const releasePort = async (port: number) => {
    try {
      const killed = await invoke<PortInfo>("kill_port", { port });
      flash(t.portReleased(killed.name, killed.pid, port));
      setPortResult("free");
    } catch (e) {
      flashErr(e);
    }
  };

  const saveShortcut = async (sc: string | null) => {
    try {
      await invoke("set_shortcut", { shortcut: sc });
      setShortcut(sc);
      flash(sc ? t.shortcutSet(prettyShortcut(sc)) : t.shortcutCleared);
    } catch (e) {
      flashErr(e);
    }
  };

  // Record the next key combo pressed while "recording" is on.
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setRecording(false);
        return;
      }
      const mods: string[] = [];
      if (e.metaKey) mods.push("super");
      if (e.ctrlKey) mods.push("ctrl");
      if (e.altKey) mods.push("alt");
      if (e.shiftKey) mods.push("shift");
      const main = codeToKey(e.code);
      // Wait until a non-modifier key arrives.
      if (!main) return;
      if (mods.length === 0) {
        flash(t.needModifier);
        return;
      }
      setRecording(false);
      saveShortcut([...mods, main].join("+"));
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording]);

  const openLogFile = async (key: string) => {
    const [projectId, ...rest] = key.split(":");
    try {
      const path = await invoke<string>("get_log_file", {
        projectId,
        script: rest.join(":"),
      });
      await invoke("reveal_in_finder", { path });
    } catch (e) {
      flashErr(e);
    }
  };

  const projectUrl = (p: Project) => {
    for (const [k, t] of running) {
      if (k.startsWith(p.id + ":") && t.url) return t.url;
    }
    return null;
  };

  const logLines = useMemo(
    () => (logKey ? logs.get(logKey) ?? [] : []),
    [logKey, logs],
  );

  const runningCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const key of running.keys()) {
      if (stopping.has(key)) continue;
      const projectId = key.split(":", 1)[0];
      counts.set(projectId, (counts.get(projectId) ?? 0) + 1);
    }
    return counts;
  }, [running, stopping]);

  const visibleProjects = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return projects;
    return projects.filter(
      (p) => p.name.toLowerCase().includes(q) || p.path.toLowerCase().includes(q),
    );
  }, [projects, search]);

  useEffect(() => {
    if (autostartBooted || projects.length === 0) return;
    setAutostartBooted(true);
    for (const p of projects) {
      for (const script of p.autostart_scripts) {
        const key = `${p.id}:${script}`;
        if (!p.pm_installed || running.has(key) || busyRef.current.has(key)) continue;
        startScript(p, script);
      }
    }
  }, [autostartBooted, projects, running]);

  return (
    <div className={`popover ${IS_MAC ? "mac" : "desktop"}`}>
      <header className="titlebar" data-tauri-drag-region>
        <div className="title-identity" data-tauri-drag-region>
          <span className="logo" data-tauri-drag-region>
            Package Run
          </span>
          <span className={`global-running ${totalRunning > 0 ? "on" : ""}`} title={totalRunning > 0 ? t.runningTasks(totalRunning) : t.noRunningTasks}>
            {totalRunning > 0 ? totalRunning : "idle"}
          </span>
        </div>
        <div className="titlebar-actions">
          {IS_MAC && (
            <button
              className={`icon-btn ${winPinned ? "active" : ""}`}
              title={winPinned ? t.unpinPanel : t.pinPanel}
              onClick={toggleWinPin}
            >
              {winPinned ? <PinOff size={14} /> : <Pin size={14} />}
            </button>
          )}
          <button className="icon-btn" title={t.addProject} onClick={addProject}>
            <Plus size={16} />
          </button>
          <button
            className={`icon-btn ${settingsOpen ? "active" : ""}`}
            title={t.settings}
            onClick={() => setSettingsOpen((open) => !open)}
          >
            <Settings size={15} />
          </button>
          <button
            className="icon-btn"
            title={t.quit}
            onClick={() => invoke("quit_app")}
          >
            <Power size={14} />
          </button>
        </div>
      </header>

      {updateInfo && (
        <div className="update-banner" role="status">
          <ArrowUpCircle size={15} className="update-banner-icon" />
          <span className="update-banner-text">
            {t.updateAvailable(updateInfo.version)}
          </span>
          <button
            className="chip small"
            onClick={() => {
              openUrl(updateInfo.htmlUrl).catch(() => openUrl(RELEASES_PAGE));
            }}
          >
            {t.updateView}
          </button>
          <button
            className="icon-btn tiny"
            title={t.updateLater}
            onClick={() => {
              dismissUpdate(updateInfo.version);
              setUpdateInfo(null);
            }}
          >
            <X size={12} />
          </button>
        </div>
      )}

      {projects.length > 3 && (
        <div className="search-bar">
          <Search className="search-icon" size={14} />
          <input
            className="search-input"
            placeholder={t.searchPlaceholder}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
      )}

      {error && <div className="toast">{error}</div>}

      <div className="content">
        {projects.length === 0 && (
          <div className="empty">
            <p>{t.noProjects}</p>
            <button className="primary" onClick={addProject}>
              {t.addProjectFolder}
            </button>
          </div>
        )}

        {visibleProjects.map((p) => {
          const count = runningCounts.get(p.id) ?? 0;
          const url = projectUrl(p);
          const isOpen = expanded === p.id;
          const sortingDisabled = search.trim().length > 0;
          return (
            <div
              key={p.id}
              className={`project ${isOpen ? "open" : ""} ${dragId === p.id ? "dragging" : ""} ${dragOverId === p.id ? "drag-over" : ""}`}
              onDragOver={(e) => {
                if (sortingDisabled || !dragId) return;
                e.preventDefault();
                if (dragOverId !== p.id) setDragOverId(p.id);
              }}
              onDrop={(e) => {
                e.preventDefault();
                if (!sortingDisabled) dropOn(p.id);
              }}
              onDragLeave={() => dragOverId === p.id && setDragOverId(null)}
            >
              <div
                className="project-row"
                onClick={() => setExpanded(isOpen ? null : p.id)}
              >
                <button
                  className="drag-handle"
                  draggable={!sortingDisabled}
                  title={sortingDisabled ? t.dragDisabledWhileSearching : t.dragSort}
                  onClick={(e) => e.stopPropagation()}
                  onDragStart={(e) => {
                    if (sortingDisabled) return;
                    e.stopPropagation();
                    setDragId(p.id);
                    e.dataTransfer.effectAllowed = "move";
                    e.dataTransfer.setData("text/plain", p.id);
                  }}
                  onDragEnd={() => {
                    setDragId(null);
                    setDragOverId(null);
                  }}
                >
                  <ChevronsUpDown size={13} />
                </button>
                <span
                  className={`run-indicator ${count > 0 ? "on" : ""}`}
                  title={count > 0 ? t.runningTasks(count) : t.noRunningTasks}
                >
                  {count > 0 ? count : ""}
                </span>
                <div className="project-meta">
                  <span className="project-name" title={p.name}>
                    {p.name}
                    {p.git_branch && (
                      <span className={`git-badge ${p.git_dirty ? "dirty" : ""}`}>
                        ⎇ {p.git_branch}
                        {p.git_dirty ? " *" : ""}
                      </span>
                    )}
                  </span>
                  <span className="project-path" title={p.path}>{p.path}</span>
                </div>
                <button
                  className={`star-btn ${p.pinned ? "on" : ""}`}
                  title={p.pinned ? t.unpin : t.pin}
                  onClick={(e) => {
                    e.stopPropagation();
                    togglePin(p.id);
                  }}
                >
                  <Star size={13} fill={p.pinned ? "currentColor" : "none"} />
                </button>
                <button
                  className={`pm-badge ${p.pm_installed ? "" : "warn"}`}
                  title={
                    p.pm_installed
                      ? t.pmSwitchHint
                      : t.pmNotInstalled(p.package_manager)
                  }
                  onClick={(e) => {
                    e.stopPropagation();
                    setExpanded(p.id);
                    setPmMenuFor(pmMenuFor === p.id ? null : p.id);
                  }}
                >
                  {p.package_manager}
                  {!p.pm_installed && " ⚠"}
                </button>
                <span className={`chevron ${isOpen ? "open" : ""}`}><ChevronRight size={20} /></span>
              </div>

              {isOpen && (
                <div className="project-detail">
                  {pmMenuFor === p.id && (
                    <div className="pm-menu">
                      <span className="pm-menu-label">{t.pmSelect}:</span>
                      {["pnpm", "yarn", "bun", "npm"].map((pm) => {
                        const available = installedPms.includes(pm);
                        const active = p.pm_overridden && p.package_manager === pm;
                        return (
                          <button
                            key={pm}
                            className={`chip small ${active ? "selected" : ""}`}
                            disabled={!available}
                            title={available ? "" : t.pmNotInstalled(pm)}
                            onClick={() => choosePm(p.id, pm)}
                          >
                            {pm}
                          </button>
                        );
                      })}
                      <button
                        className={`chip small ${p.pm_overridden ? "" : "selected"}`}
                        onClick={() => choosePm(p.id, null)}
                      >
                        {t.pmAuto}
                      </button>
                    </div>
                  )}
                  <div className="quick-actions">
                    {url && (
                      <button className="chip link" onClick={() => openUrl(url)}>
                        <ExternalLink size={12} />
                        {url.replace(/^https?:\/\//, "")}
                      </button>
                    )}
                    <button
                      className="chip"
                      onClick={() => invoke("open_in_editor", { path: p.path }).catch((e) => flashErr(e))}
                    >
                      <MonitorUp size={12} />
                      {t.editor}
                    </button>
                    <button
                      className="chip"
                      onClick={() => invoke("open_in_terminal", { path: p.path }).catch((e) => flashErr(e))}
                    >
                      <TerminalSquare size={12} />
                      {t.terminal}
                    </button>
                    <button
                      className="chip"
                      onClick={() => invoke("reveal_in_finder", { path: p.path })}
                    >
                      <FolderOpen size={12} />
                      {t.finder}
                    </button>
                    <button className="chip danger" onClick={() => removeProject(p.id)}>
                      <Trash2 size={12} />
                      {t.remove}
                    </button>
                  </div>

                  <div className="scripts">
                    {orderedScripts(p, running).map((name) => {
                      const key = `${p.id}:${name}`;
                      const isStopping = stopping.has(key);
                      const isRunning = running.has(key) && !isStopping;
                      const hasCfg = !!p.launch[name];
                      const startsAtLogin = p.autostart_scripts.includes(name);
                      return (
                        <div key={name}>
                          <div className="script-row">
                            <button
                              className={`run-btn ${isRunning ? "stop" : ""} ${isStopping ? "loading" : ""}`}
                              title={isStopping ? t.stopping : isRunning ? t.stop : t.run}
                              disabled={isStopping}
                              onClick={() =>
                                isRunning ? stopScript(p, name) : startScript(p, name)
                              }
                            >
                              {isStopping ? <RotateCw size={12} /> : isRunning ? <Square size={10} fill="currentColor" /> : <Play size={12} fill="currentColor" />}
                            </button>
                            <span
                              className={`script-name ${isRunning ? "active" : ""} ${isStopping ? "stopping" : ""}`}
                              title={p.scripts[name]}
                            >
                              {name}
                            </span>
                            {isRunning && !isStopping && (
                              <button
                                className="chip small"
                                title={t.restart}
                                onClick={() => restartScript(p, name)}
                              >
                                <RotateCw size={12} />
                              </button>
                            )}
                            {(isRunning || (logs.get(key)?.length ?? 0) > 0) && (
                              <button
                                className={`chip small ${logKey === key ? "selected" : ""}`}
                                onClick={() => setLogKey(logKey === key ? null : key)}
                              >
                                {t.logs}
                              </button>
                            )}
                            <button
                              className={`chip small ${startsAtLogin ? "selected" : ""}`}
                              title={t.scriptAutostartTitle}
                              onClick={() => toggleScriptAutostart(p, name)}
                            >
                              {t.scriptAutostart}
                            </button>
                            <button
                              className={`gear-btn ${hasCfg ? "has-config" : ""} ${cfgFor === key ? "open" : ""}`}
                              title={t.configure}
                              onClick={() => openCfgEditor(p, name)}
                            >
                              <Settings size={13} />
                            </button>
                          </div>

                          {cfgFor === key && (
                            <div className="cfg-editor">
                              <div className="cfg-row">
                                <label>{t.cfgPort}</label>
                                <input
                                  className="port-input"
                                  inputMode="numeric"
                                  placeholder="3000"
                                  value={cfgPort}
                                  onChange={(e) =>
                                    setCfgPort(e.target.value.replace(/\D/g, ""))
                                  }
                                />
                                <label>{t.cfgEnvFile}</label>
                                <select
                                  className="cfg-select"
                                  value={cfgEnvFile}
                                  onChange={(e) => setCfgEnvFile(e.target.value)}
                                >
                                  <option value="">{t.cfgEnvFileNone}</option>
                                  {p.env_files.map((f) => (
                                    <option key={f} value={f}>
                                      {f}
                                    </option>
                                  ))}
                                </select>
                              </div>
                              <label className="cfg-label">{t.cfgEnv}</label>
                              <textarea
                                className="cfg-textarea"
                                rows={3}
                                spellCheck={false}
                                placeholder={"API_BASE=http://localhost:8080\nMOCK=1"}
                                value={cfgEnv}
                                onChange={(e) => setCfgEnv(e.target.value)}
                              />
                              <div className="cfg-row">
                                <label>{t.cfgArgs}</label>
                                <input
                                  className="cfg-input"
                                  placeholder="--host 0.0.0.0"
                                  spellCheck={false}
                                  value={cfgArgs}
                                  onChange={(e) => setCfgArgs(e.target.value)}
                                />
                              </div>
                              <div className="cfg-actions">
                                <button
                                  className="chip small selected"
                                  onClick={() => saveCfg(p, name)}
                                >
                                  {t.cfgSave}
                                </button>
                                {hasCfg && (
                                  <button
                                    className="chip small danger"
                                    onClick={() => clearCfg(p, name)}
                                  >
                                    {t.cfgClear}
                                  </button>
                                )}
                              </div>
                            </div>
                          )}
                        </div>
                      );
                    })}
                    {Object.keys(p.scripts).length === 0 && (
                      <div className="no-scripts">
                        {p.exists ? t.noScriptsInPkg : t.noPkgJson}
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      <div className="footer">
        {settingsOpen && (
          <div className="settings-panel">
            <div className="settings-section">
              <div className="settings-heading">{t.settingsGeneral}</div>
              <label className="settings-toggle">
                <input type="checkbox" checked={autoStart} onChange={toggleAutostart} />
                <span>
                  <strong>{t.autoStart}</strong>
                  <small>{t.autoStartDesc}</small>
                </span>
              </label>
              <div className="settings-row">
                <button
                  className={`chip small ${recording ? "recording" : ""}`}
                  title={shortcut ? t.reRecordTitle : t.recordShortcutTitle}
                  onClick={() => setRecording(!recording)}
                >
                  <Keyboard size={12} />
                  {recording ? t.pressShortcut : shortcut ? prettyShortcut(shortcut) : t.setShortcut}
                </button>
                {shortcut && !recording && (
                  <button
                    className="icon-btn tiny"
                    title={t.clearShortcut}
                    onClick={() => saveShortcut(null)}
                  >
                    <X size={12} />
                  </button>
                )}
              </div>
            </div>

            <div className="settings-section">
              <div className="settings-heading">{t.settingsAppearance}</div>
              <div className="settings-row">
                <button
                  className="chip small"
                  title={t.themeTitle(themeLabel())}
                  onClick={toggleTheme}
                >
                  {theme === "light" ? <Sun size={12} /> : <Moon size={12} />}
                  {themeLabel()}
                </button>
                <button
                  className="chip small"
                  title={lang === "zh" ? "Switch to English" : "切换为中文"}
                  onClick={toggleLang}
                >
                  <Globe2 size={12} />
                  {lang === "zh" ? "EN" : "中"}
                </button>
              </div>
            </div>

            <div className="settings-section">
              <div className="settings-heading">{t.settingsTools}</div>
              <div className="port-checker inline">
                <input
                  className="port-input"
                  placeholder={t.portPlaceholder}
                  inputMode="numeric"
                  value={portQuery}
                  onChange={(e) => {
                    setPortQuery(e.target.value.replace(/\D/g, ""));
                    setPortResult(null);
                  }}
                  onKeyDown={(e) => e.key === "Enter" && queryPort(portQuery)}
                />
                <button className="chip small" onClick={() => queryPort(portQuery)}>
                  {t.query}
                </button>
              </div>
              {portResult && (
                <div className="settings-row port-result">
                  {portResult === "free" ? (
                    <span className="hint">{t.portFree(portQuery)}</span>
                  ) : (
                    <>
                      <span className="hint occupied">
                        {t.portOccupiedBy(portResult.name, portResult.pid, portResult.port)}
                      </span>
                      <button
                        className="chip small danger"
                        onClick={() => releasePort(portResult.port)}
                      >
                        {t.releasePort}
                      </button>
                    </>
                  )}
                </div>
              )}
            </div>

            <div className="settings-section">
              <div className="settings-heading">{t.updateSection}</div>
              <div className="settings-row">
                <span className="hint">
                  {appVersion ? t.updateCurrent(appVersion) : "…"}
                </span>
              </div>
              <div className="settings-row">
                <button
                  className="chip small"
                  disabled={updateChecking}
                  onClick={() => runUpdateCheck({ ignoreDismissed: true, manual: true })}
                >
                  <ArrowUpCircle size={12} />
                  {updateChecking ? t.updateChecking : t.updateCheck}
                </button>
                <button
                  className="chip small ghost"
                  onClick={() => openUrl(RELEASES_PAGE)}
                >
                  <ExternalLink size={12} />
                  {t.updateOpenReleases}
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

      {logKey && (
        <div className="log-panel">
          <div className="log-header">
            <span>{logKey.split(":").slice(1).join(":")} {t.logsTitle}</span>
            <div className="log-header-actions">
              <button className="chip small ghost" onClick={() => openLogFile(logKey)}>
                {t.openLogFile}
              </button>
              <button className="icon-btn" onClick={() => setLogKey(null)}>
                <X size={14} />
              </button>
            </div>
          </div>
          <div className="log-body" ref={logRef}>
            {logLines.length === 0 ? (
              <div className="log-line dim">{t.waitingOutput}</div>
            ) : (
              logLines.map((l, i) => (
                <div key={i} className="log-line">
                  {renderLogLine(l)}
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
