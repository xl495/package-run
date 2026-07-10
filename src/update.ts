/** Update check: prefers Rust backend (reliable network), falls back to GitHub API fetch. */

import { invoke } from "@tauri-apps/api/core";

export const RELEASES_PAGE = "https://github.com/xl495/package-run/releases";
export const LATEST_API =
  "https://api.github.com/repos/xl495/package-run/releases/latest";

const DISMISS_KEY = "package-run-dismissed-update";

export interface AvailableUpdate {
  version: string;
  notes: string;
  htmlUrl: string;
}

/** Compare SemVer (leading `v` optional). Returns true if remote > local. */
export function isNewerVersion(remote: string, local: string): boolean {
  const r = parseSemver(remote);
  const l = parseSemver(local);
  if (!r || !l) return remote.replace(/^v/i, "") !== local.replace(/^v/i, "");
  for (let i = 0; i < 3; i++) {
    if (r[i] > l[i]) return true;
    if (r[i] < l[i]) return false;
  }
  return false;
}

function parseSemver(v: string): [number, number, number] | null {
  const m = v.trim().replace(/^v/i, "").match(/^(\d+)\.(\d+)\.(\d+)/);
  if (!m) return null;
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

export function getDismissedVersion(): string | null {
  return localStorage.getItem(DISMISS_KEY);
}

export function dismissUpdate(version: string) {
  localStorage.setItem(DISMISS_KEY, version.replace(/^v/i, ""));
}

async function fetchLatestViaJs(): Promise<AvailableUpdate | null> {
  const res = await fetch(LATEST_API, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "Package-Run-Updater",
    },
  });
  if (!res.ok) throw new Error(`GitHub API ${res.status}`);
  const data = (await res.json()) as {
    tag_name?: string;
    name?: string;
    body?: string;
    html_url?: string;
    draft?: boolean;
    prerelease?: boolean;
  };
  if (data.draft || data.prerelease) return null;
  const version = (data.tag_name ?? data.name ?? "").replace(/^v/i, "");
  if (!version) return null;
  return {
    version,
    notes: (data.body ?? "").trim(),
    htmlUrl: data.html_url ?? `${RELEASES_PAGE}/tag/v${version}`,
  };
}

async function fetchLatestViaRust(
  currentVersion: string,
): Promise<AvailableUpdate | null> {
  return invoke<AvailableUpdate | null>("check_app_update", {
    currentVersion,
  });
}

export async function checkForUpdate(
  currentVersion: string,
  opts?: { ignoreDismissed?: boolean },
): Promise<AvailableUpdate | null> {
  let latest: AvailableUpdate | null = null;
  try {
    latest = await fetchLatestViaRust(currentVersion);
  } catch {
    const remote = await fetchLatestViaJs();
    if (remote && isNewerVersion(remote.version, currentVersion)) {
      latest = remote;
    }
  }

  if (!latest) return null;
  if (!opts?.ignoreDismissed) {
    const dismissed = getDismissedVersion();
    if (dismissed && dismissed === latest.version.replace(/^v/i, "")) {
      return null;
    }
  }
  return latest;
}
