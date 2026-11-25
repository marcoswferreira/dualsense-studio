use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::emulation::EmulationConfig;
use crate::haptics_audio::HapticsConfig;
use crate::lightbar::LightingConfig;
use crate::mapping::InputMappingConfig;
use crate::profiles::TriggerProfile;
use crate::triggers::DualTriggerConfig;

/// Automatic per-game profile switching rule (matched against running process names)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameProfileRule {
    pub process_name: String,
    pub profile_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_rumble_gain() -> f32 {
    1.0
}

/// Complete persistent standalone configuration for DualSense Studio
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_true")]
    pub extension_enabled: bool,
    #[serde(default)]
    pub autostart_on_launch: bool,
    #[serde(default)]
    pub triggers: DualTriggerConfig,
    #[serde(default)]
    pub lighting: LightingConfig,
    #[serde(default)]
    pub emulation: EmulationConfig,
    #[serde(default)]
    pub mapping: InputMappingConfig,
    #[serde(default)]
    pub haptics: HapticsConfig,
    #[serde(default = "default_rumble_gain")]
    pub rumble_gain: f32,
    #[serde(default)]
    pub udp_enabled: bool,
    #[serde(default)]
    pub cemuhook_enabled: bool,
    #[serde(default)]
    pub custom_profiles: Vec<TriggerProfile>,
    #[serde(default)]
    pub game_profile_rules: Vec<GameProfileRule>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            extension_enabled: true,
            autostart_on_launch: false,
            triggers: DualTriggerConfig::default(),
            lighting: LightingConfig::default(),
            emulation: EmulationConfig::default(),
            mapping: InputMappingConfig::default(),
            haptics: HapticsConfig::default(),
            rumble_gain: 1.0,
            udp_enabled: false,
            cemuhook_enabled: false,
            custom_profiles: Vec::new(),
            game_profile_rules: Vec::new(),
        }
    }
}

/// Returns the default OS-appropriate config file path (`<config_dir>/dualsense-studio/config.json`)
pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("dualsense-studio")
        .join("config.json")
}

impl AppConfig {
    /// Loads configuration from `path`, falling back to `Default` if missing or malformed
    pub fn load_from_path(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(raw) => match serde_json::from_str::<AppConfig>(&raw) {
                Ok(mut cfg) => {
                    cfg.rumble_gain = cfg.rumble_gain.clamp(0.0, 2.0);
                    info!(
                        "Loaded DualSense Studio configuration from {}",
                        path.display()
                    );
                    cfg
                }
                Err(e) => {
                    warn!(
                        "Config file {} is malformed ({}); using defaults",
                        path.display(),
                        e
                    );
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Atomically persists configuration to `path` via temp file + rename
    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json)?;
        // On Windows, fs::rename fails if destination exists; remove existing cleanly first
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightbar::RgbColor;
    use crate::triggers::TriggerEffect;

    #[test]
    fn test_config_atomic_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("config.json");

        let mut cfg = AppConfig {
            rumble_gain: 1.4,
            cemuhook_enabled: true,
            ..Default::default()
        };
        cfg.triggers.right = TriggerEffect::Weapon {
            start_position: 2,
            end_position: 6,
            force: 8,
        };
        cfg.custom_profiles.push(TriggerProfile {
            id: "my_custom".to_string(),
            name: "My Custom".to_string(),
            description: "Custom shooter".to_string(),
            triggers: cfg.triggers.clone(),
            lightbar: RgbColor::PURPLE,
        });
        cfg.game_profile_rules.push(GameProfileRule {
            process_name: "Cyberpunk2077.exe".to_string(),
            profile_id: "my_custom".to_string(),
            enabled: true,
        });

        cfg.save_to_path(&path).expect("save config");
        let loaded = AppConfig::load_from_path(&path);
        assert_eq!(loaded, cfg);

        // Overwrite works cleanly on Windows & Linux
        cfg.rumble_gain = 0.75;
        cfg.save_to_path(&path).expect("overwrite config");
        let reloaded = AppConfig::load_from_path(&path);
        assert!((reloaded.rumble_gain - 0.75).abs() < 1e-5);
    }

    #[test]
    fn test_config_corrupt_file_falls_back_to_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");
        std::fs::write(&path, "{ invalid json !!!").unwrap();

        let loaded = AppConfig::load_from_path(&path);
        assert_eq!(loaded, AppConfig::default());
    }
}
