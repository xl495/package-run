use serde::Serialize;

const LATEST_API: &str = "https://api.github.com/repos/xl495/package-run/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/xl495/package-run/releases";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: String,
    pub html_url: String,
}

#[derive(Debug, serde::Deserialize)]
struct GhRelease {
    tag_name: Option<String>,
    name: Option<String>,
    body: Option<String>,
    html_url: Option<String>,
    draft: Option<bool>,
    prerelease: Option<bool>,
}

fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let s = v.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts
        .next()
        .and_then(|p| {
            let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().ok()
        })
        .unwrap_or(0);
    Some((major, minor, patch))
}

/// Returns true if remote is strictly newer than local.
pub fn is_newer(remote: &str, local: &str) -> bool {
    match (parse_semver(remote), parse_semver(local)) {
        (Some(r), Some(l)) => r > l,
        _ => {
            remote.trim().trim_start_matches(['v', 'V'])
                != local.trim().trim_start_matches(['v', 'V'])
        }
    }
}

/// Check GitHub Releases for a newer version than `current_version`.
/// Returns `Ok(None)` when already up to date (or only draft/prerelease).
#[tauri::command]
pub fn check_app_update(current_version: String) -> Result<Option<AvailableUpdate>, String> {
    let body = ureq::get(LATEST_API)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "Package-Run-Updater")
        .timeout(std::time::Duration::from_secs(12))
        .call()
        .map_err(|e| format!("update check failed: {e}"))?
        .into_string()
        .map_err(|e| format!("update response read failed: {e}"))?;

    let release: GhRelease =
        serde_json::from_str(&body).map_err(|e| format!("update parse failed: {e}"))?;

    if release.draft.unwrap_or(false) || release.prerelease.unwrap_or(false) {
        return Ok(None);
    }

    let version = release
        .tag_name
        .or(release.name)
        .unwrap_or_default()
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_string();

    if version.is_empty() {
        return Ok(None);
    }

    if !is_newer(&version, &current_version) {
        return Ok(None);
    }

    let html_url = release
        .html_url
        .unwrap_or_else(|| format!("{RELEASES_PAGE}/tag/v{version}"));

    Ok(Some(AvailableUpdate {
        version,
        notes: release.body.unwrap_or_default().trim().to_string(),
        html_url,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_newer() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }
}
