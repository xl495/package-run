export type Lang = "zh" | "en";

const zh = {
  pinPanel: "固定面板（点击外部不收起）",
  unpinPanel: "取消固定（点击外部会收起）",
  addProject: "添加项目",
  settings: "设置",
  quit: "退出",
  searchPlaceholder: "搜索项目…",
  noProjects: "还没有项目",
  addProjectFolder: "添加一个项目文件夹",
  themeSystem: "跟随系统",
  themeLight: "浅色模式",
  themeDark: "深色模式",
  themeTitle: (theme: string) => `当前：${theme}，点击切换`,
  runningTasks: (count: number) => `正在运行 ${count} 个任务`,
  noRunningTasks: "没有运行中的任务",
  editor: "编辑器",
  terminal: "终端",
  finder: "访达",
  remove: "移除",
  run: "运行",
  stop: "停止",
  stopping: "正在停止…",
  restart: "重启",
  logs: "日志",
  logsTitle: "日志",
  openLogFile: "打开日志文件",
  waitingOutput: "等待输出…",
  noScriptsInPkg: "package.json 中没有 scripts",
  noPkgJson: "找不到 package.json",
  pin: "置顶",
  unpin: "取消置顶",
  dragSort: "拖动调整排序",
  dragDisabledWhileSearching: "搜索时不能拖动排序",
  autoStart: "开机自启",
  autoStartDesc: "登录后自动打开 Package Run",
  scriptAutostart: "开机运行",
  scriptAutostartTitle: "登录后自动启动这个脚本",
  settingsTitle: "设置",
  settingsGeneral: "通用",
  settingsAppearance: "外观与语言",
  settingsTools: "工具",
  setShortcut: "设置快捷键",
  pressShortcut: "按下快捷键…",
  recordShortcutTitle: "录制一个全局快捷键",
  reRecordTitle: "点击重新录制，Esc 取消",
  clearShortcut: "清除快捷键",
  needModifier: "请包含至少一个修饰键（⌘ / ⌃ / ⌥ / ⇧）",
  shortcutSet: (sc: string) => `快捷键已设为 ${sc}`,
  shortcutCleared: "快捷键已清除",
  portPlaceholder: "端口",
  query: "查询",
  invalidPort: "请输入有效端口号",
  portFree: (port: string) => `端口 ${port} 空闲`,
  portOccupiedBy: (name: string, pid: number, port: number) =>
    `${name}（pid ${pid}）占用了 ${port}`,
  releasePort: "释放端口",
  portReleased: (name: string, pid: number, port: number) =>
    `已结束 ${name}（pid ${pid}），端口 ${port} 已释放`,
  portConflictDetected: (port: string) => `端口 ${port} 被占用，已在下方定位占用进程`,
  scriptCrashed: (script: string, code: number) =>
    `「${script}」异常退出（代码 ${code}），点击「日志」查看原因`,
  pmNotInstalled: (pm: string) => `项目偏好 ${pm}，但本地未安装`,
  pmAuto: "自动",
  pmSelect: "包管理器",
  pmSwitchHint: "点击切换包管理器",
  configure: "启动配置",
  cfgPort: "端口",
  cfgEnv: "环境变量（每行一条 KEY=VALUE）",
  cfgEnvFile: "加载 env 文件",
  cfgEnvFileNone: "不加载",
  cfgArgs: "附加参数",
  cfgSave: "保存",
  cfgClear: "清除配置",
  cfgSaved: "启动配置已保存",
  cfgCleared: "启动配置已清除",
  cfgInvalidEnv: (line: string) => `环境变量格式错误：${line}`,
  // backend error codes
  errPmMissing: (pm: string) =>
    `本地未安装 ${pm}。可点击包管理器徽标切换，或运行 corepack enable ${pm} 安装`,
  errAlreadyRunning: "该脚本已在运行",
  errNotRunning: "脚本未在运行",
  errNoPackageJson: "所选目录中没有 package.json，不是一个前端项目",
  errSpawn: (pm: string, detail: string) => `启动 ${pm} 失败：${detail}`,
  errLogFile: (detail: string) => `无法创建日志文件：${detail}`,
  errNoLogFile: "日志文件不存在",
  errPortFree: "该端口未被占用",
  errKillFail: (name: string, pid: string) => `无法结束进程 ${name}（pid ${pid}）`,
  errNoEditor: "未找到可用的编辑器",
  errNoTerminal: "无法打开终端",
  errShortcut: (detail: string) => `注册快捷键失败：${detail}`,
  // updates
  updateAvailable: (version: string) => `发现新版本 v${version}`,
  updateView: "查看更新",
  updateLater: "稍后",
  updateCheck: "检查更新",
  updateChecking: "检查中…",
  updateLatest: (version: string) => `已是最新版 v${version}`,
  updateFailed: "检查更新失败，请稍后重试",
  updateSection: "关于",
  updateCurrent: (version: string) => `当前版本 v${version}`,
  updateOpenReleases: "打开下载页",
};

const en: typeof zh = {
  pinPanel: "Pin panel (stays open on outside click)",
  unpinPanel: "Unpin panel (hides on outside click)",
  addProject: "Add project",
  settings: "Settings",
  quit: "Quit",
  searchPlaceholder: "Search projects…",
  noProjects: "No projects yet",
  addProjectFolder: "Add a project folder",
  themeSystem: "System",
  themeLight: "Light",
  themeDark: "Dark",
  themeTitle: (theme: string) => `Current: ${theme}. Click to switch`,
  runningTasks: (count: number) => `${count} task${count === 1 ? "" : "s"} running`,
  noRunningTasks: "No running tasks",
  editor: "Editor",
  terminal: "Terminal",
  finder: "Finder",
  remove: "Remove",
  run: "Run",
  stop: "Stop",
  stopping: "Stopping…",
  restart: "Restart",
  logs: "Logs",
  logsTitle: "logs",
  openLogFile: "Open log file",
  waitingOutput: "Waiting for output…",
  noScriptsInPkg: "No scripts in package.json",
  noPkgJson: "package.json not found",
  pin: "Pin to top",
  unpin: "Unpin",
  dragSort: "Drag to reorder",
  dragDisabledWhileSearching: "Reordering is disabled while searching",
  autoStart: "Launch at login",
  autoStartDesc: "Open Package Run automatically after login",
  scriptAutostart: "Run at login",
  scriptAutostartTitle: "Automatically start this script after login",
  settingsTitle: "Settings",
  settingsGeneral: "General",
  settingsAppearance: "Appearance & language",
  settingsTools: "Tools",
  setShortcut: "Set shortcut",
  pressShortcut: "Press keys…",
  recordShortcutTitle: "Record a global shortcut",
  reRecordTitle: "Click to re-record, Esc to cancel",
  clearShortcut: "Clear shortcut",
  needModifier: "Include at least one modifier (⌘ / ⌃ / ⌥ / ⇧)",
  shortcutSet: (sc: string) => `Shortcut set to ${sc}`,
  shortcutCleared: "Shortcut cleared",
  portPlaceholder: "Port",
  query: "Check",
  invalidPort: "Enter a valid port number",
  portFree: (port: string) => `Port ${port} is free`,
  portOccupiedBy: (name: string, pid: number, port: number) =>
    `${name} (pid ${pid}) is using ${port}`,
  releasePort: "Free port",
  portReleased: (name: string, pid: number, port: number) =>
    `Killed ${name} (pid ${pid}), port ${port} is free`,
  portConflictDetected: (port: string) =>
    `Port ${port} is in use — culprit located below`,
  scriptCrashed: (script: string, code: number) =>
    `"${script}" exited with code ${code} — check its logs`,
  pmNotInstalled: (pm: string) => `Project prefers ${pm}, but it isn't installed`,
  pmAuto: "Auto",
  pmSelect: "Package manager",
  pmSwitchHint: "Click to switch package manager",
  configure: "Launch config",
  cfgPort: "Port",
  cfgEnv: "Env vars (KEY=VALUE per line)",
  cfgEnvFile: "Load env file",
  cfgEnvFileNone: "None",
  cfgArgs: "Extra args",
  cfgSave: "Save",
  cfgClear: "Clear config",
  cfgSaved: "Launch config saved",
  cfgCleared: "Launch config cleared",
  cfgInvalidEnv: (line: string) => `Invalid env line: ${line}`,
  errPmMissing: (pm: string) =>
    `${pm} is not installed. Click the package manager badge to switch, or run: corepack enable ${pm}`,
  errAlreadyRunning: "This script is already running",
  errNotRunning: "Script is not running",
  errNoPackageJson: "No package.json in the selected folder",
  errSpawn: (pm: string, detail: string) => `Failed to start ${pm}: ${detail}`,
  errLogFile: (detail: string) => `Cannot create log file: ${detail}`,
  errNoLogFile: "Log file does not exist",
  errPortFree: "Port is not in use",
  errKillFail: (name: string, pid: string) => `Cannot kill ${name} (pid ${pid})`,
  errNoEditor: "No editor found",
  errNoTerminal: "Cannot open a terminal",
  errShortcut: (detail: string) => `Failed to register shortcut: ${detail}`,
  updateAvailable: (version: string) => `Update available: v${version}`,
  updateView: "View update",
  updateLater: "Later",
  updateCheck: "Check for updates",
  updateChecking: "Checking…",
  updateLatest: (version: string) => `You're on the latest version v${version}`,
  updateFailed: "Could not check for updates",
  updateSection: "About",
  updateCurrent: (version: string) => `Version v${version}`,
  updateOpenReleases: "Open releases",
};

export const STRINGS: Record<Lang, typeof zh> = { zh, en };

const LANG_KEY = "package-run-lang";

export function detectLang(): Lang {
  const saved = localStorage.getItem(LANG_KEY);
  if (saved === "zh" || saved === "en") return saved;
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

export function persistLang(lang: Lang) {
  localStorage.setItem(LANG_KEY, lang);
}

/** Backend errors arrive as "CODE|param|param"; translate known codes. */
export function translateError(raw: string, t: typeof zh): string {
  const [code, ...params] = String(raw).split("|");
  switch (code) {
    case "ERR_PM_MISSING":
      return t.errPmMissing(params[0] ?? "");
    case "ERR_ALREADY_RUNNING":
      return t.errAlreadyRunning;
    case "ERR_NOT_RUNNING":
      return t.errNotRunning;
    case "ERR_NO_PACKAGE_JSON":
      return t.errNoPackageJson;
    case "ERR_SPAWN":
      return t.errSpawn(params[0] ?? "", params.slice(1).join("|"));
    case "ERR_LOG_FILE":
      return t.errLogFile(params.join("|"));
    case "ERR_NO_LOG_FILE":
      return t.errNoLogFile;
    case "ERR_PORT_FREE":
      return t.errPortFree;
    case "ERR_KILL":
      return t.errKillFail(params[0] ?? "", params[1] ?? "");
    case "ERR_NO_EDITOR":
      return t.errNoEditor;
    case "ERR_NO_TERMINAL":
      return t.errNoTerminal;
    case "ERR_SHORTCUT":
      return t.errShortcut(params.join("|"));
    default:
      return raw;
  }
}
