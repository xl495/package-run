# Package Run

[English](./README.md) · [中文](./README.zh.md)

**Keywords:** frontend project manager · local dev server runner · menu bar app · system tray · Tauri · React · pnpm/npm/yarn/bun · Laravel Herd alternative for JavaScript

**Package Run** is a lightweight **local frontend project manager** built with **Tauri 2**.  
On macOS it lives in the menu bar (similar to [Laravel Herd](https://herd.laravel.com/)); on Windows / Linux it runs as a normal window with a system tray icon.

Manage multiple frontend dev servers in one place: **start / stop / restart** `package.json` scripts, stream logs, open `localhost`, jump into your editor or terminal — without juggling terminal tabs.

| | |
| --- | --- |
| **Repo** | https://github.com/xl495/package-run |
| **Releases / download** | https://github.com/xl495/package-run/releases |
| **Latest** | [v0.1.0](https://github.com/xl495/package-run/releases/tag/v0.1.0) |
| **Stack** | Tauri 2 · React 19 · TypeScript · Vite · Rust |

## Features

| Feature | Description |
| --- | --- |
| Menu bar / tray | macOS: `›_` menu bar icon + global shortcut (`⌥Space` by default). Windows/Linux: window + system tray (closing the window minimizes to tray) |
| Panel pin | Pin the panel so it stays open when clicking outside — useful for watching logs |
| Project management | Add local folders with a `package.json`; search, pin, and drag to reorder |
| Package managers | Auto-detect pnpm / yarn / bun / npm (lockfile + `packageManager` field); manual override supported |
| Git status | Shows current branch and dirty working tree indicator |
| Script control | Start / stop / restart any script; stop kills the whole process tree (no orphans) |
| Live logs | ~500 lines in the panel; full logs on disk under `logs/`; clickable URLs |
| Local preview | Detects `localhost` URLs in output and opens them in the browser |
| Port tools | Port occupancy check & one-click release; auto-locate the process on `EADDRINUSE` |
| Quick open | Open in Cursor / VS Code / Zed / WebStorm, iTerm / Warp / Terminal, or Finder |
| Launch config | Per-script port, env vars, env file, and extra args |
| Autostart | App launch at login; optional per-script autostart |
| Appearance | Chinese / English UI; light / dark / system theme |

## Screenshots

<!-- Add screenshots after publishing the repo -->

## Tech stack

- **Tauri 2** (Rust: process management, log streaming, tray, global shortcuts)
- **React 19 + TypeScript + Vite 7**
- Plugins: `positioner` · `dialog` · `autostart` · `global-shortcut` · `opener`

## Development

Requirements:

- Node.js 20+
- [pnpm](https://pnpm.io/) 9+
- [Rust](https://www.rust-lang.org/) stable
- System deps: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

```bash
pnpm install
pnpm tauri dev
```

## Local build

```bash
pnpm tauri build
```

Artifacts: `src-tauri/target/release/bundle/`

| Platform | Typical outputs |
| --- | --- |
| macOS | `macos/Package Run.app`, `dmg/Package Run_*.dmg` |
| Windows | `msi/`, `nsis/` |
| Linux | `deb/`, `appimage/` |

## GitHub automated builds

GitHub Actions (`tauri-apps/tauri-action`) builds for **macOS (Apple Silicon + Intel), Windows, and Linux**, then attaches artifacts to a Release.

### Triggers

1. **Tag release (recommended)**

   ```bash
   # Keep package.json / tauri.conf.json / Cargo.toml versions in sync, e.g. 0.2.0
   git tag v0.2.0
   git push origin v0.2.0
   ```

2. **Manual run**: GitHub → Actions → **Release** → Run workflow

3. **Push to the `release` branch** also triggers the workflow

The job creates a **Draft Release** with installers under Assets. Review and click Publish when ready.

### Repository permissions

If you see `Resource not accessible by integration`:

**Settings → Actions → General → Workflow permissions** → enable **Read and write permissions**.

### macOS code signing

CI is **not** configured with an Apple Developer certificate. Unsigned Apple Silicon builds may be reported as “damaged”. Users can run:

```bash
xattr -cr "/Applications/Package Run.app"
```

For production distribution, follow the [Tauri macOS signing guide](https://v2.tauri.app/distribute/sign/macos/).

## Data storage

| Platform | Path |
| --- | --- |
| macOS | `~/Library/Application Support/com.huangxinliang.packagerun/` |
| Windows | `%APPDATA%\com.huangxinliang.packagerun\` |
| Linux | `~/.local/share/com.huangxinliang.packagerun/` |

Main files: `projects.json` (project list & config), `settings.json` (shortcuts, etc.).

## License

[MIT](./LICENSE)
