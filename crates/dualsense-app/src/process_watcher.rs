use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dualsense_core::{DualSenseManager, GameProfileRule};
use tokio::task::JoinHandle;
use tracing::info;

/// Matches a list of running process names against configured `GameProfileRule`s (case-insensitive)
pub fn match_running_process_rule<'a>(
    running_processes: &[String],
    rules: &'a [GameProfileRule],
) -> Option<&'a GameProfileRule> {
    for rule in rules {
        if !rule.enabled || rule.process_name.trim().is_empty() {
            continue;
        }
        let target = rule.process_name.trim().to_ascii_lowercase();
        if running_processes
            .iter()
            .any(|p| p.to_ascii_lowercase() == target || p.to_ascii_lowercase().ends_with(&target))
        {
            return Some(rule);
        }
    }
    None
}

/// Enumerates currently running OS process names (lightweight, no extra dependencies)
pub fn list_running_processes() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let Ok(out) = std::process::Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output()
        else {
            return Vec::new();
        };
        let text = String::from_utf8_lossy(&out.stdout);
        text.lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                let first = trimmed.split(',').next()?;
                Some(first.trim_matches('"').to_string())
            })
            .filter(|s| !s.is_empty())
            .collect()
    }
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let s = fname.to_string_lossy();
            if s.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) {
                    let name = comm.trim().to_string();
                    if !name.is_empty() {
                        names.push(name);
                    }
                }
            }
        }
        names
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Vec::new()
    }
}

/// Background watcher that automatically switches DualSense profiles when a configured game starts
pub struct ProcessProfileWatcher {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ProcessProfileWatcher {
    pub fn start(manager: Arc<DualSenseManager>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();

        let handle = tokio::spawn(async move {
            let mut last_applied: Option<String> = None;
            let mut tick = tokio::time::interval(Duration::from_secs(3));
            while flag.load(Ordering::Relaxed) {
                tick.tick().await;
                if !manager.is_extension_enabled() {
                    continue;
                }
                let rules = manager.get_game_profile_rules();
                if rules.is_empty() {
                    continue;
                }
                let procs = tokio::task::spawn_blocking(list_running_processes)
                    .await
                    .unwrap_or_default();
                if let Some(matched) = match_running_process_rule(&procs, &rules) {
                    if last_applied.as_deref() != Some(matched.profile_id.as_str())
                        && manager.apply_profile(&matched.profile_id)
                    {
                        info!(
                            "Auto-switched to profile '{}' for detected game '{}'",
                            matched.profile_id, matched.process_name
                        );
                        last_applied = Some(matched.profile_id.clone());
                    }
                } else {
                    last_applied = None;
                }
            }
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

impl Drop for ProcessProfileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_running_process_rule_case_insensitive_and_enabled_filter() {
        let procs = vec![
            "explorer.exe".to_string(),
            "Cyberpunk2077.exe".to_string(),
            "discord.exe".to_string(),
        ];
        let rules = vec![
            GameProfileRule {
                process_name: "cyberpunk2077.exe".to_string(),
                profile_id: "disabled_one".to_string(),
                enabled: false,
            },
            GameProfileRule {
                process_name: "CYBERPUNK2077.EXE".to_string(),
                profile_id: "fps_tactical".to_string(),
                enabled: true,
            },
        ];

        let matched =
            match_running_process_rule(&procs, &rules).expect("should match enabled rule");
        assert_eq!(matched.profile_id, "fps_tactical");
    }
}
