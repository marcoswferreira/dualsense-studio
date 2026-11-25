use serde::{Deserialize, Serialize};

/// Sony DualSense Adaptive Trigger Effect Modes (Full DualSenseX 19-mode parity)
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TriggerEffect {
    /// No adaptive resistance applied (Normal / Off)
    #[default]
    #[serde(alias = "normal")]
    Off,
    /// Continuous rigid resistance starting from specified position (Resistance / Feedback)
    #[serde(alias = "resistance")]
    Feedback {
        /// Position where resistance begins (0-9)
        start_position: u8,
        /// Resistance force (0-8)
        force: u8,
    },
    /// Weapon / Semi-Auto tactile snap (high resistance until threshold, then breaks through)
    #[serde(alias = "semi_automatic_gun")]
    Weapon {
        /// Position where trigger resistance begins (0-9)
        start_position: u8,
        /// Position where trigger breaks through / fires (0-9)
        end_position: u8,
        /// Resistance force before break (0-8)
        force: u8,
    },
    /// Bow tension simulation (progressive force until release)
    Bow {
        /// Start of draw (0-9)
        start_position: u8,
        /// Fully drawn position (0-9)
        end_position: u8,
        /// Initial pull force (0-8)
        force: u8,
        /// Force at full draw (0-8)
        snap_force: u8,
    },
    /// Galloping effect with alternating resistance pulses
    Galloping {
        start_position: u8,
        end_position: u8,
        first_foot: u8,
        second_foot: u8,
        frequency: u8,
    },
    /// Automatic Gun / Machine Gun rhythmic recoil kicks
    #[serde(alias = "automatic_gun")]
    MachineGun {
        start_position: u8,
        end_position: u8,
        strength: u8,
        frequency: u8,
    },
    /// DualSenseX Machine effect (dual-amplitude periodic oscillation)
    Machine {
        start_position: u8,
        end_position: u8,
        strength_a: u8,
        strength_b: u8,
        frequency: u8,
        period: u8,
    },
    /// Vibration / Pulse at specific position with amplitude and frequency
    #[serde(alias = "vibrate_trigger")]
    Vibration {
        position: u8,
        amplitude: u8,
        frequency: u8,
    },
    /// DualSenseX GameCube trigger (smooth analog travel ending in a hard digital click barrier)
    GameCube,
    /// DualSenseX Choppy segmented resistance steps across travel
    Choppy,
    /// DualSenseX Preset: Very Soft resistance
    VerySoft,
    /// DualSenseX Preset: Soft resistance
    Soft,
    /// DualSenseX Preset: Medium resistance
    Medium,
    /// DualSenseX Preset: Hard resistance
    Hard,
    /// DualSenseX Preset: Very Hard resistance
    VeryHard,
    /// DualSenseX Preset: Hardest resistance
    Hardest,
    /// DualSenseX Preset: Rigid full-stroke wall resistance
    Rigid,
    /// DualSenseX Preset: Vibrate Trigger Pulse
    VibrateTriggerPulse,
    /// Custom 10-zone resistance profile (CustomTriggerValue)
    #[serde(alias = "custom_trigger_value")]
    CustomZone { zones: [u8; 10] },
}

impl TriggerEffect {
    /// Returns a human-readable label for logging and diagnostics
    pub fn name(&self) -> &'static str {
        match self {
            TriggerEffect::Off => "Off/Normal",
            TriggerEffect::Feedback { .. } => "Feedback/Resistance",
            TriggerEffect::Weapon { .. } => "Weapon/SemiAutoGun",
            TriggerEffect::Bow { .. } => "Bow",
            TriggerEffect::Galloping { .. } => "Galloping",
            TriggerEffect::MachineGun { .. } => "MachineGun/AutoGun",
            TriggerEffect::Machine { .. } => "Machine",
            TriggerEffect::Vibration { .. } => "Vibration",
            TriggerEffect::GameCube => "GameCube",
            TriggerEffect::Choppy => "Choppy",
            TriggerEffect::VerySoft => "VerySoft",
            TriggerEffect::Soft => "Soft",
            TriggerEffect::Medium => "Medium",
            TriggerEffect::Hard => "Hard",
            TriggerEffect::VeryHard => "VeryHard",
            TriggerEffect::Hardest => "Hardest",
            TriggerEffect::Rigid => "Rigid",
            TriggerEffect::VibrateTriggerPulse => "VibrateTriggerPulse",
            TriggerEffect::CustomZone { .. } => "CustomTriggerValue",
        }
    }

    /// Compiles the trigger effect into the official 11-byte Sony DualSense firmware HID payload
    /// (bit-packed zone masks per Sony SDK / Nielk1 `TriggerEffectGenerator` specification).
    pub fn compile_payload(&self) -> [u8; 11] {
        let mut bytes = [0u8; 11];
        match self {
            TriggerEffect::Off => {
                bytes[0] = 0x05;
            }
            TriggerEffect::Feedback {
                start_position,
                force,
            } => {
                if *force == 0 {
                    bytes[0] = 0x05;
                    return bytes;
                }
                let start = (*start_position).min(9);
                let force_val = ((*force).clamp(1, 8) - 1) & 0x07;
                let mut active_zones: u16 = 0;
                let mut force_zones: u32 = 0;
                for i in start..10 {
                    active_zones |= 1u16 << i;
                    force_zones |= (force_val as u32) << (3 * i);
                }
                bytes[0] = 0x21;
                bytes[1..3].copy_from_slice(&active_zones.to_le_bytes());
                bytes[3..7].copy_from_slice(&force_zones.to_le_bytes());
            }
            TriggerEffect::Weapon {
                start_position,
                end_position,
                force,
            } => {
                if *force == 0 {
                    bytes[0] = 0x05;
                    return bytes;
                }
                let start = (*start_position).min(8);
                let end = (*end_position).clamp(start + 1, 9);
                let start_and_stop: u16 = (1u16 << start) | (1u16 << end);
                let force_val = ((*force).clamp(1, 8) - 1) & 0x07;
                bytes[0] = 0x25;
                bytes[1..3].copy_from_slice(&start_and_stop.to_le_bytes());
                bytes[3] = force_val;
            }
            TriggerEffect::Bow {
                start_position,
                end_position,
                force,
                snap_force,
            } => {
                let start = (*start_position).min(8);
                let end = (*end_position).clamp(start + 1, 9);
                let start_and_stop: u16 = (1u16 << start) | (1u16 << end);
                let f = ((*force).clamp(1, 8) - 1) & 0x07;
                let sf = ((*snap_force).clamp(1, 8) - 1) & 0x07;
                bytes[0] = 0x22;
                bytes[1..3].copy_from_slice(&start_and_stop.to_le_bytes());
                bytes[3] = (sf << 3) | f;
            }
            TriggerEffect::Galloping {
                start_position,
                end_position,
                first_foot,
                second_foot,
                frequency,
            } => {
                let start = (*start_position).min(8);
                let end = (*end_position).clamp(start + 1, 9);
                let start_and_stop: u16 = (1u16 << start) | (1u16 << end);
                let ff = (*first_foot).min(6);
                let sf = (*second_foot).clamp(ff + 1, 7);
                bytes[0] = 0x23;
                bytes[1..3].copy_from_slice(&start_and_stop.to_le_bytes());
                bytes[3] = (sf << 3) | ff;
                bytes[4] = (*frequency).max(1);
            }
            TriggerEffect::MachineGun {
                start_position,
                end_position,
                strength,
                frequency,
            } => {
                if *strength == 0 || *frequency == 0 {
                    bytes[0] = 0x05;
                    return bytes;
                }
                let start = (*start_position).min(9);
                let end = (*end_position).clamp(start, 9);
                let amp_val = ((*strength).clamp(1, 8) - 1) & 0x07;
                let mut active_zones: u16 = 0;
                let mut amp_zones: u32 = 0;
                for i in start..=end {
                    active_zones |= 1u16 << i;
                    amp_zones |= (amp_val as u32) << (3 * i);
                }
                bytes[0] = 0x26;
                bytes[1..3].copy_from_slice(&active_zones.to_le_bytes());
                bytes[3..7].copy_from_slice(&amp_zones.to_le_bytes());
                bytes[9] = *frequency;
            }
            TriggerEffect::Machine {
                start_position,
                end_position,
                strength_a,
                strength_b,
                frequency,
                period,
            } => {
                let start = (*start_position).min(8);
                let end = (*end_position).clamp(start + 1, 9);
                let start_and_stop: u16 = (1u16 << start) | (1u16 << end);
                let sa = (*strength_a).min(7);
                let sb = (*strength_b).min(7);
                bytes[0] = 0x27;
                bytes[1..3].copy_from_slice(&start_and_stop.to_le_bytes());
                bytes[3] = (sb << 3) | sa;
                bytes[4] = (*frequency).max(1);
                bytes[5] = *period;
            }
            TriggerEffect::Vibration {
                position,
                amplitude,
                frequency,
            } => {
                if *amplitude == 0 || *frequency == 0 {
                    bytes[0] = 0x05;
                    return bytes;
                }
                let pos = (*position).min(9);
                let amp_level = if *amplitude <= 8 {
                    (*amplitude - 1) & 0x07
                } else {
                    (((*amplitude).min(63) as u16 * 7) / 63) as u8 & 0x07
                };
                let mut active_zones: u16 = 0;
                let mut amp_zones: u32 = 0;
                for i in pos..10 {
                    active_zones |= 1u16 << i;
                    amp_zones |= (amp_level as u32) << (3 * i);
                }
                bytes[0] = 0x26;
                bytes[1..3].copy_from_slice(&active_zones.to_le_bytes());
                bytes[3..7].copy_from_slice(&amp_zones.to_le_bytes());
                bytes[9] = *frequency;
            }
            TriggerEffect::GameCube => {
                bytes[0] = 0x02;
                bytes[1] = 0x90;
                bytes[2] = 0xA0;
                bytes[3] = 0xFF;
            }
            TriggerEffect::Choppy => {
                bytes[0] = 0x02;
                bytes[1] = 0x02;
                bytes[2] = 0x27;
                bytes[3] = 0x03;
                bytes[4] = 0x27;
                bytes[5] = 0x26;
                bytes[6] = 0x02;
            }
            TriggerEffect::VerySoft => {
                bytes[0] = 0x01;
                bytes[1] = 0x80;
                bytes[2] = 0x20;
            }
            TriggerEffect::Soft => {
                bytes[0] = 0x01;
                bytes[1] = 0x45;
                bytes[2] = 0xA0;
            }
            TriggerEffect::Medium => {
                bytes[0] = 0x02;
                bytes[1] = 0x02;
                bytes[2] = 0x01;
                bytes[3] = 0x02;
                bytes[4] = 0x02;
                bytes[5] = 0x01;
                bytes[6] = 0x22;
                bytes[7] = 0xFF;
            }
            TriggerEffect::Hard => {
                bytes[0] = 0x01;
                bytes[1] = 0x20;
                bytes[2] = 0xFF;
            }
            TriggerEffect::VeryHard => {
                bytes[0] = 0x01;
                bytes[1] = 0x10;
                bytes[2] = 0xFF;
            }
            TriggerEffect::Hardest => {
                bytes[0] = 0x01;
                bytes[1] = 0x00;
                bytes[2] = 0xFF;
                bytes[3] = 0xFF;
                bytes[4] = 0xFF;
                bytes[5] = 0xFF;
                bytes[6] = 0xFF;
            }
            TriggerEffect::Rigid => {
                bytes[0] = 0x01;
                bytes[1] = 0x00;
                bytes[2] = 0xFF;
            }
            TriggerEffect::VibrateTriggerPulse => {
                bytes[0] = 0x02;
                bytes[1] = 0x25;
                bytes[2] = 0x06;
                bytes[3] = 0x02;
                bytes[4] = 0x27;
                bytes[5] = 0x25;
                bytes[6] = 0x23;
            }
            TriggerEffect::CustomZone { zones } => {
                let mut active_zones: u16 = 0;
                let mut force_zones: u32 = 0;
                for (i, &z) in zones.iter().enumerate() {
                    if z > 0 {
                        let level = if z <= 8 {
                            (z - 1) & 0x07
                        } else {
                            (((z as u16) * 7) / 255) as u8 & 0x07
                        };
                        active_zones |= 1u16 << i;
                        force_zones |= (level as u32) << (3 * i);
                    }
                }
                if active_zones == 0 {
                    bytes[0] = 0x05;
                } else {
                    bytes[0] = 0x21;
                    bytes[1..3].copy_from_slice(&active_zones.to_le_bytes());
                    bytes[3..7].copy_from_slice(&force_zones.to_le_bytes());
                }
            }
        }
        bytes
    }
}

/// Dual trigger configuration for Left (L2) and Right (R2) triggers
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DualTriggerConfig {
    pub left: TriggerEffect,
    pub right: TriggerEffect,
}

impl DualTriggerConfig {
    pub fn new(left: TriggerEffect, right: TriggerEffect) -> Self {
        Self { left, right }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_off() {
        let effect = TriggerEffect::Off;
        let payload = effect.compile_payload();
        assert_eq!(payload[0], 0x05);
        assert_eq!(&payload[1..], &[0u8; 10]);
        assert_eq!(effect.name(), "Off/Normal");
    }

    #[test]
    fn test_trigger_feedback() {
        let effect = TriggerEffect::Feedback {
            start_position: 2,
            force: 7,
        };
        let payload = effect.compile_payload();
        assert_eq!(payload[0], 0x21);
        // Active zones 2..10 = 0b11_1111_1100 = 0x03FC
        let active_zones = u16::from_le_bytes([payload[1], payload[2]]);
        assert_eq!(active_zones, 0x03FC);
        let force_zones = u32::from_le_bytes([payload[3], payload[4], payload[5], payload[6]]);
        // Force 7 encodes as 6 (0..7) in each active 3-bit slot
        assert_eq!((force_zones >> 6) & 0x07, 6);
        assert_eq!((force_zones >> 27) & 0x07, 6);
        assert_eq!(effect.name(), "Feedback/Resistance");
    }

    #[test]
    fn test_trigger_weapon() {
        let effect = TriggerEffect::Weapon {
            start_position: 1,
            end_position: 6,
            force: 8,
        };
        let payload = effect.compile_payload();
        assert_eq!(payload[0], 0x25);
        let mask = u16::from_le_bytes([payload[1], payload[2]]);
        assert_eq!(mask, (1 << 1) | (1 << 6));
        assert_eq!(payload[3], 7); // force 8 encodes as 7
        assert_eq!(effect.name(), "Weapon/SemiAutoGun");
    }

    #[test]
    fn test_trigger_bow() {
        let effect = TriggerEffect::Bow {
            start_position: 0,
            end_position: 8,
            force: 4,
            snap_force: 7,
        };
        let payload = effect.compile_payload();
        assert_eq!(payload[0], 0x22);
        let mask = u16::from_le_bytes([payload[1], payload[2]]);
        assert_eq!(mask, (1 << 0) | (1 << 8));
        // force=4 -> 3, snap_force=7 -> 6 => (6 << 3) | 3 = 0x33
        assert_eq!(payload[3], (6 << 3) | 3);
        assert_eq!(effect.name(), "Bow");
    }

    #[test]
    fn test_trigger_galloping_and_machine() {
        let gal = TriggerEffect::Galloping {
            start_position: 1,
            end_position: 8,
            first_foot: 3,
            second_foot: 6,
            frequency: 15,
        };
        let p_gal = gal.compile_payload();
        assert_eq!(p_gal[0], 0x23);
        assert_eq!(
            u16::from_le_bytes([p_gal[1], p_gal[2]]),
            (1 << 1) | (1 << 8)
        );
        assert_eq!(p_gal[3], (6 << 3) | 3);
        assert_eq!(p_gal[4], 15);
        assert_eq!(gal.name(), "Galloping");

        let mach = TriggerEffect::Machine {
            start_position: 1,
            end_position: 9,
            strength_a: 4,
            strength_b: 6,
            frequency: 20,
            period: 2,
        };
        let p_mach = mach.compile_payload();
        assert_eq!(p_mach[0], 0x27);
        assert_eq!(
            u16::from_le_bytes([p_mach[1], p_mach[2]]),
            (1 << 1) | (1 << 9)
        );
        assert_eq!(p_mach[3], (6 << 3) | 4);
        assert_eq!(p_mach[4], 20);
        assert_eq!(p_mach[5], 2);
        assert_eq!(mach.name(), "Machine");
    }

    #[test]
    fn test_trigger_dualsensex_presets() {
        assert_eq!(TriggerEffect::GameCube.compile_payload()[0], 0x02);
        assert_eq!(TriggerEffect::GameCube.compile_payload()[1], 0x90);
        assert_eq!(TriggerEffect::Choppy.compile_payload()[0], 0x02);
        assert_eq!(TriggerEffect::VerySoft.compile_payload()[0], 0x01);
        assert_eq!(TriggerEffect::Soft.compile_payload()[0], 0x01);
        assert_eq!(TriggerEffect::Medium.compile_payload()[0], 0x02);
        assert_eq!(TriggerEffect::Hard.compile_payload()[0], 0x01);
        assert_eq!(TriggerEffect::VeryHard.compile_payload()[0], 0x01);
        assert_eq!(TriggerEffect::Hardest.compile_payload()[3], 0xFF);
        assert_eq!(TriggerEffect::Rigid.compile_payload()[2], 0xFF);
        assert_eq!(
            TriggerEffect::VibrateTriggerPulse.compile_payload()[0],
            0x02
        );
    }

    #[test]
    fn test_trigger_custom_zone() {
        let zones = [1, 2, 3, 4, 5, 6, 7, 8, 8, 8];
        let effect = TriggerEffect::CustomZone { zones };
        let payload = effect.compile_payload();
        assert_eq!(payload[0], 0x21);
        assert_eq!(u16::from_le_bytes([payload[1], payload[2]]), 0x03FF);
        let force_zones = u32::from_le_bytes([payload[3], payload[4], payload[5], payload[6]]);
        assert_eq!(force_zones & 0x07, 0); // zone 0 = 1 -> 0
        assert_eq!((force_zones >> 21) & 0x07, 7); // zone 7 = 8 -> 7
    }

    #[test]
    fn test_trigger_serialization() {
        let config = DualTriggerConfig {
            left: TriggerEffect::Feedback {
                start_position: 3,
                force: 5,
            },
            right: TriggerEffect::Weapon {
                start_position: 2,
                end_position: 7,
                force: 6,
            },
        };
        let json = serde_json::to_string(&config).unwrap();
        let decoded: DualTriggerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, decoded);
    }
}
