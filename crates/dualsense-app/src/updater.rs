use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::watch;
use tracing::{info, warn};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/marcoswferreira/dualsense-studio/releases/latest";
const GITHUB_RELEASES_LIST_URL: &str =
    "https://api.github.com/repos/marcoswferreira/dualsense-studio/releases?per_page=1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub release_notes: String,
}

/// Parses a version string like `v0.2.1` or `0.2.1-alpha` into `(major, minor, patch)`.
pub fn parse_semver_tuple(raw: &str) -> Option<(u64, u64, u64)> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    let core = trimmed.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor, patch))
}

/// Returns `true` if `latest` represents a newer semantic version than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver_tuple(current), parse_semver_tuple(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    }
}

/// Selects the best release asset `(name, browser_download_url)` for the current OS.
pub fn select_platform_asset(assets: &[serde_json::Value]) -> Option<(String, String)> {
    let mut candidates: Vec<(String, String)> = assets
        .iter()
        .filter_map(|a| {
            let name = a.get("name")?.as_str()?.to_string();
            let url = a.get("browser_download_url")?.as_str()?.to_string();
            Some((name, url))
        })
        .collect();

    if cfg!(target_os = "windows") {
        if let Some(setup) = candidates
            .iter()
            .find(|(n, _)| n.ends_with("-windows-x86_64-setup.exe"))
        {
            return Some(setup.clone());
        }
        if let Some(zip) = candidates
            .iter()
            .find(|(n, _)| n.ends_with("-windows-x86_64.zip"))
        {
            return Some(zip.clone());
        }
    } else if cfg!(target_os = "linux") {
        if let Some(tar) = candidates
            .iter()
            .find(|(n, _)| n.ends_with("-linux-x86_64.tar.gz"))
        {
            return Some(tar.clone());
        }
    }

    candidates.pop()
}

async fn fetch_json_url(url: &str) -> Result<serde_json::Value> {
    let curl_bin = if cfg!(target_os = "windows") {
        "curl.exe"
    } else {
        "curl"
    };
    let output = Command::new(curl_bin)
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: DualSense-Studio-Updater",
            url,
        ])
        .output()
        .await
        .with_context(|| format!("Failed to execute {curl_bin} for update check"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("HTTP request to {url} failed: {stderr}"));
    }

    serde_json::from_slice(&output.stdout).context("Invalid JSON from GitHub Releases API")
}

/// Checks GitHub Releases for a newer version of DualSense Studio.
pub async fn check_for_updates() -> Result<UpdateInfo> {
    let release = match fetch_json_url(GITHUB_RELEASES_LATEST_URL).await {
        Ok(val) => val,
        Err(_) => {
            let list = fetch_json_url(GITHUB_RELEASES_LIST_URL).await?;
            list.as_array()
                .and_then(|arr| arr.first().cloned())
                .ok_or_else(|| anyhow!("No GitHub releases found"))?
        }
    };

    let tag_name = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Release missing tag_name"))?
        .to_string();
    let release_url = release
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://github.com/marcoswferreira/dualsense-studio/releases")
        .to_string();
    let release_notes = release
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let assets = release
        .get("assets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let (asset_name, asset_url) = match select_platform_asset(&assets) {
        Some((n, u)) => (Some(n), Some(u)),
        None => (None, None),
    };

    let update_available = is_newer_version(CURRENT_VERSION, &tag_name);

    Ok(UpdateInfo {
        current_version: CURRENT_VERSION.to_string(),
        latest_version: tag_name,
        update_available,
        release_url,
        asset_name,
        asset_url,
        release_notes,
    })
}

async fn download_file(url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let curl_bin = if cfg!(target_os = "windows") {
        "curl.exe"
    } else {
        "curl"
    };
    let dest_str = dest
        .to_str()
        .ok_or_else(|| anyhow!("Non-UTF8 destination path"))?;
    let output = Command::new(curl_bin)
        .args([
            "-fSL",
            "--max-time",
            "120",
            "-H",
            "User-Agent: DualSense-Studio-Updater",
            "-o",
            dest_str,
            url,
        ])
        .output()
        .await
        .with_context(|| format!("Failed to download {url}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Download failed: {stderr}"));
    }
    Ok(())
}

/// Downloads and launches the platform update package, then triggers graceful shutdown.
pub async fn apply_update(
    info: &UpdateInfo,
    shutdown_tx: Option<&watch::Sender<bool>>,
) -> Result<String> {
    let Some(asset_url) = info.asset_url.as_deref() else {
        return Err(anyhow!(
            "No downloadable release asset found for this platform; visit {}",
            info.release_url
        ));
    };

    let asset_name = info
        .asset_name
        .as_deref()
        .unwrap_or("dualsense-studio-update.bin");
    let update_dir = std::env::temp_dir().join("dualsense-studio-update");
    let target_file = update_dir.join(asset_name);

    info!(
        "Downloading DualSense Studio update {} from {}",
        info.latest_version, asset_url
    );
    download_file(asset_url, &target_file).await?;

    #[cfg(target_os = "windows")]
    {
        if asset_name.ends_with(".exe") {
            let file_str = target_file.to_string_lossy().to_string();
            std::process::Command::new("cmd")
                .args([
                    "/C",
                    "start",
                    "",
                    &file_str,
                    "/SP-",
                    "/SILENT",
                    "/CLOSEAPPLICATIONS",
                ])
                .spawn()
                .context("Failed to launch Windows setup installer")?;

            if let Some(tx) = shutdown_tx {
                let _ = tx.send(true);
            }
            return Ok(format!(
                "Launched installer for {} ({}).",
                info.latest_version, asset_name
            ));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if asset_name.ends_with(".tar.gz") {
            let extract_dir = update_dir.join("extracted");
            let _ = tokio::fs::remove_dir_all(&extract_dir).await;
            tokio::fs::create_dir_all(&extract_dir).await?;
            let status = Command::new("tar")
                .args([
                    "-xzf",
                    &target_file.to_string_lossy(),
                    "-C",
                    &extract_dir.to_string_lossy(),
                ])
                .status()
                .await?;
            if !status.success() {
                return Err(anyhow!("Failed to extract Linux update archive"));
            }

            if let Ok(exe_path) = std::env::current_exe() {
                let pid = std::process::id();
                let script_path = update_dir.join("apply-update.sh");
                let script = format!(
                    "#!/usr/bin/env bash\nset -e\nwhile kill -0 {pid} 2>/dev/null; do sleep 0.2; done\nNEW_BIN=$(find \"{}\" -type f -name dualsense-studio | head -n 1)\nif [ -n \"$NEW_BIN\" ]; then\n  cp \"$NEW_BIN\" \"{}\"\n  chmod +x \"{}\"\nfi\nNEW_UI=$(find \"{}\" -type d -name ui | head -n 1)\nif [ -n \"$NEW_UI\" ] && [ -d \"{}\" ]; then\n  cp -r \"$NEW_UI\"/* \"{}\"/\nfi\nnohup \"{}\" >/dev/null 2>&1 &\n",
                    extract_dir.display(),
                    exe_path.display(),
                    exe_path.display(),
                    extract_dir.display(),
                    exe_path.parent().unwrap_or(&update_dir).join("ui").display(),
                    exe_path.parent().unwrap_or(&update_dir).join("ui").display(),
                    exe_path.display()
                );
                tokio::fs::write(&script_path, script).await?;
                std::process::Command::new("bash")
                    .arg(&script_path)
                    .spawn()
                    .context("Failed to spawn Linux update helper")?;

                if let Some(tx) = shutdown_tx {
                    let _ = tx.send(true);
                }
                return Ok(format!(
                    "Applying update {} and restarting...",
                    info.latest_version
                ));
            }
        }
    }

    warn!(
        "Downloaded update to {}, manual install required",
        target_file.display()
    );
    Ok(format!(
        "Downloaded update {} to {}",
        info.latest_version,
        target_file.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_semver_parsing_and_comparison() {
        assert_eq!(parse_semver_tuple("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_semver_tuple("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver_tuple("v0.2.0-alpha"), Some((0, 2, 0)));
        assert!(is_newer_version("0.1.0", "v0.1.1"));
        assert!(is_newer_version("0.1.0", "v0.2.0"));
        assert!(!is_newer_version("0.1.0", "v0.1.0"));
        assert!(!is_newer_version("0.2.0", "v0.1.9"));
    }

    #[test]
    fn test_select_platform_asset_prefers_setup_on_windows_and_tarball_on_linux() {
        let assets = vec![
            json!({
                "name": "dualsense-studio-v0.2.0-linux-x86_64.tar.gz",
                "browser_download_url": "https://example.com/linux.tar.gz"
            }),
            json!({
                "name": "dualsense-studio-v0.2.0-windows-x86_64.zip",
                "browser_download_url": "https://example.com/win.zip"
            }),
            json!({
                "name": "dualsense-studio-v0.2.0-windows-x86_64-setup.exe",
                "browser_download_url": "https://example.com/win-setup.exe"
            }),
        ];

        let (name, _url) = select_platform_asset(&assets).expect("asset selected");
        if cfg!(target_os = "windows") {
            assert!(name.ends_with("-windows-x86_64-setup.exe"));
        } else if cfg!(target_os = "linux") {
            assert!(name.ends_with("-linux-x86_64.tar.gz"));
        }
    }
}
