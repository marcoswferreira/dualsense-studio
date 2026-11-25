import React, { useState, useEffect, useCallback, useMemo } from 'react';
import {
  BatteryCharging,
  BatteryFull,
  Bluetooth,
  Cpu,
  FolderKanban,
  Gamepad2,
  Globe,
  Mic,
  MicOff,
  Power,
  Radio,
  SlidersHorizontal,
  Sparkles,
  Usb,
  Waves,
  Zap,
} from 'lucide-react';
import { useTranslation } from '../../i18n';
import {
  DualSenseStatus,
  DualTriggerConfig,
  LightingConfig,
  EmulationConfig,
  InputMappingConfig,
  HapticsConfig,
  TriggerProfile,
  GameProfileRule,
} from '../../types/models';
import { TriggerStudio } from './TriggerStudio';
import { LightingStudio } from './LightingStudio';
import { EmulationStudio } from './EmulationStudio';
import { MappingStudio } from './MappingStudio';
import { HapticsAudioStudio } from './HapticsAudioStudio';
import { UdpServerStudio } from './UdpServerStudio';
import { GameProfilesStudio } from './GameProfilesStudio';
import { ControllerTester } from './ControllerTester';
import { webHidController, WebHidDualSenseController } from '../../utils/dualsenseHardware';

type SubTab =
  | 'tester'
  | 'triggers'
  | 'lighting'
  | 'mapping'
  | 'emulation'
  | 'haptics'
  | 'udp'
  | 'profiles';

interface Props {
  onShowToast?: (message: string) => void;
}

const DEFAULT_STATUS: DualSenseStatus = {
  extension_enabled: true,
  autostart_on_launch: false,
  connected: false,
  name: 'Disconnected',
  connection_type: 'usb',
  battery: { percentage: 0, is_charging: false, is_full: false },
  triggers: { left: { mode: 'off' }, right: { mode: 'off' } },
  lighting: {
    lightbar: { mode: 'static', color: { r: 0, g: 68, b: 204 } },
    player_led: 'player1',
    mic_led: 'off',
  },
  emulation: {
    mode: 'native',
    gyro: { enabled: false, sensitivity: 1.0, deadzone: 50, invert_x: false, invert_y: false },
  },
  udp_stats: {
    running: false,
    port: 6969,
    packets_received: 0,
    last_packet_time: null,
    last_sender: null,
  },
  profiles: [],
  mic_muted: false,
  audio_device_detected: false,
  speaker_volume: 80,
  mapping: {
    left_stick: { inner_deadzone: 0.05, outer_deadzone: 1.0, anti_deadzone: 0.0, curve: { type: 'linear' }, invert_x: false, invert_y: false },
    right_stick: { inner_deadzone: 0.05, outer_deadzone: 1.0, anti_deadzone: 0.0, curve: { type: 'linear' }, invert_x: false, invert_y: false },
    left_trigger: { deadzone: 0, max_threshold: 255, sensitivity: 1.0 },
    right_trigger: { deadzone: 0, max_threshold: 255, sensitivity: 1.0 },
    button_remaps: {},
    touchpad: { mode: 'passthrough', sensitivity: 1.0, tap_to_click: true, two_finger_scroll: true, invert_scroll: false },
    gyro_activation: 'always',
    gyro_smoothing: 0.0,
    gyro_calibration_offset: [0, 0, 0],
  },
  haptics: {
    mode: 'legacy_rumble',
    enabled: false,
    gain: 1.0,
    lowpass_cutoff_hz: 180,
    left_balance: 1.0,
    right_balance: 1.0,
    audio_source: 'default_loopback',
  },
  haptics_metrics: {
    active: false,
    packets_sent: 0,
    left_level: 0,
    right_level: 0,
    sample_rate_hz: 3000,
  },
  rumble_gain: 1.0,
  cemuhook_stats: {
    running: false,
    port: 26760,
    active_clients: 0,
    packets_sent: 0,
    packets_received: 0,
  },
  virtual_driver: {
    available: false,
    active: false,
    backend_name: 'none',
    vigem_installed: false,
    hidhide_installed: false,
    detail: 'Backend offline',
  },
  game_profile_rules: [],
};

export const DualSenseTab: React.FC<Props> = ({ onShowToast }) => {
  const { t, lang, toggleLang } = useTranslation();
  const [activeSubTab, setActiveSubTab] = useState<SubTab>('tester');
  const [status, setStatus] = useState<DualSenseStatus | null>(null);
  const [isWebHidConnected, setIsWebHidConnected] = useState(webHidController.isConnected());

  useEffect(() => {
    const timer = window.setInterval(() => {
      setIsWebHidConnected(webHidController.isConnected());
    }, 900);
    return () => window.clearInterval(timer);
  }, []);

  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch('/api/dualsense/status');
      if (res.ok) {
        const data: DualSenseStatus = await res.json();
        setStatus(data);
      }
    } catch {
      // Backend offline — graceful fallback
    }
  }, []);

  useEffect(() => {
    void fetchStatus();
    const interval = window.setInterval(() => void fetchStatus(), 2500);
    return () => window.clearInterval(interval);
  }, [fetchStatus]);

  const postJson = useCallback(
    async (url: string, body?: unknown, successToast?: string, method: 'POST' | 'DELETE' = 'POST') => {
      try {
        const res = await fetch(url, {
          method,
          headers: body !== undefined ? { 'Content-Type': 'application/json' } : undefined,
          body: body !== undefined ? JSON.stringify(body) : undefined,
        });
        if (res.ok) {
          if (successToast) onShowToast?.(successToast);
          void fetchStatus();
          return await res.json().catch(() => ({}));
        }
      } catch {
        onShowToast?.(t('toast_error'));
      }
      return null;
    },
    [fetchStatus, onShowToast, t]
  );

  const currentStatus = status ?? DEFAULT_STATUS;
  const isHardwareConnected = currentStatus.connected || isWebHidConnected;

  const ambientColor = useMemo(() => {
    const lb = currentStatus.lighting.lightbar;
    if (lb.mode === 'static' || lb.mode === 'breathing') {
      return `rgb(${lb.color.r}, ${lb.color.g}, ${lb.color.b})`;
    }
    if (lb.mode === 'off') return '#334155';
    return '#3b82f6';
  }, [currentStatus.lighting.lightbar]);

  const handleConnectWebHid = async () => {
    const ok = await webHidController.connect();
    setIsWebHidConnected(ok);
    if (ok) {
      onShowToast?.(t('toast_webhid_connected'));
      await webHidController.sendReport(currentStatus.triggers, currentStatus.lighting);
    } else {
      onShowToast?.(t('toast_webhid_failed'));
    }
  };

  const handleDisconnectWebHid = async () => {
    await webHidController.disconnect();
    setIsWebHidConnected(false);
    onShowToast?.(t('toast_webhid_disconnected'));
  };

  const handleApplyTriggers = async (triggers: DualTriggerConfig) => {
    if (webHidController.isConnected()) {
      await webHidController.sendReport(triggers, currentStatus.lighting);
    }
    await postJson('/api/dualsense/trigger', triggers, t('toast_dualsense_triggers_applied'));
  };

  const handleApplyLighting = async (lighting: LightingConfig) => {
    if (webHidController.isConnected()) {
      await webHidController.sendReport(currentStatus.triggers, lighting);
    }
    await postJson('/api/dualsense/lightbar', lighting, t('toast_dualsense_lighting_applied'));
  };

  const handleToggleMic = async () => {
    const data = await postJson('/api/dualsense/mic/toggle');
    if (data) {
      onShowToast?.(data.mic_muted ? t('toast_mic_muted') : t('toast_mic_unmuted'));
    }
  };

  const displayControllerName = currentStatus.connected
    ? currentStatus.name
    : isWebHidConnected
      ? 'Sony DualSense (WebHID)'
      : lang === 'pt'
        ? 'Aguardando Controle DualSense'
        : 'Waiting for DualSense Controller';

  const navItems: { id: SubTab; label: string; icon: React.ReactNode; badge?: string; activeBadge?: boolean }[] = [
    {
      id: 'tester',
      label: t('dualsense_subtab_tester'),
      icon: <Gamepad2 size={16} />,
      badge: isHardwareConnected ? 'LIVE' : undefined,
      activeBadge: isHardwareConnected,
    },
    { id: 'triggers', label: t('dualsense_subtab_triggers'), icon: <Zap size={16} /> },
    { id: 'lighting', label: t('dualsense_subtab_lighting'), icon: <Sparkles size={16} /> },
    {
      id: 'mapping',
      label: t('dualsense_subtab_mapping'),
      icon: <SlidersHorizontal size={16} />,
      badge: currentStatus.cemuhook_stats.running ? 'DSU' : undefined,
      activeBadge: currentStatus.cemuhook_stats.running,
    },
    {
      id: 'emulation',
      label: t('dualsense_subtab_emulation'),
      icon: <Cpu size={16} />,
      badge: currentStatus.emulation.mode !== 'native' ? currentStatus.emulation.mode.toUpperCase().slice(0, 4) : undefined,
      activeBadge: currentStatus.emulation.mode !== 'native',
    },
    {
      id: 'haptics',
      label: t('dualsense_subtab_haptics'),
      icon: <Waves size={16} />,
      badge: currentStatus.haptics_metrics.active ? '0x32' : undefined,
      activeBadge: currentStatus.haptics_metrics.active,
    },
    {
      id: 'udp',
      label: t('dualsense_subtab_udp'),
      icon: <Radio size={16} />,
      badge: currentStatus.udp_stats.running ? 'ON' : undefined,
      activeBadge: currentStatus.udp_stats.running,
    },
    {
      id: 'profiles',
      label: t('dualsense_subtab_profiles'),
      icon: <FolderKanban size={16} />,
      badge: String(currentStatus.profiles.length || 4),
    },
  ];

  return (
    <div className="studio-shell">
      {/* LEFT NAVIGATION RAIL */}
      <aside className="studio-rail">
        <div className="rail-brand">
          <div className="brand-mark">
            <Gamepad2 size={18} />
          </div>
          <div className="brand-text">
            <div className="brand-name">DualSense Studio</div>
            <div className="brand-tag">STANDALONE v0.1</div>
          </div>
        </div>

        <nav className="rail-nav" aria-label="Primary">
          {navItems.map((item) => (
            <button
              key={item.id}
              className={`rail-item ${activeSubTab === item.id ? 'active' : ''}`}
              onClick={() => setActiveSubTab(item.id)}
              title={item.label}
            >
              <span className="rail-item-left">
                {item.icon}
                <span className="rail-label">{item.label}</span>
              </span>
              {item.badge && (
                <span className={`rail-badge ${item.activeBadge ? 'on' : ''}`}>{item.badge}</span>
              )}
            </button>
          ))}
        </nav>

        <div className="rail-footer">
          <label className="check-label rail-footer-label">
            <input
              type="checkbox"
              checked={currentStatus.autostart_on_launch}
              onChange={(e) =>
                void postJson('/api/dualsense/extension/toggle', { autostart: e.target.checked })
              }
            />
            {lang === 'pt' ? 'Iniciar automaticamente' : 'Autostart engine'}
          </label>

          <div className="rail-footer-row">
            <button
              className={`btn sm ${currentStatus.extension_enabled ? 'danger' : 'primary'}`}
              style={{ flex: 1 }}
              onClick={() =>
                void postJson('/api/dualsense/extension/toggle', {
                  enabled: !currentStatus.extension_enabled,
                })
              }
              title={currentStatus.extension_enabled ? 'Pause Engine' : 'Start Engine'}
            >
              <Power size={13} />
              <span className="rail-label">
                {currentStatus.extension_enabled ? (lang === 'pt' ? 'Ativo' : 'Active') : (lang === 'pt' ? 'Pausado' : 'Paused')}
              </span>
            </button>
            <button className="btn sm" onClick={toggleLang} title="Switch Language">
              <Globe size={13} />
              <span className="rail-label">{lang.toUpperCase()}</span>
            </button>
          </div>
        </div>
      </aside>

      {/* MAIN STUDIO AREA */}
      <div className="studio-main">
        {/* TOP LIVE TELEMETRY HEADER */}
        <header className="studio-topbar">
          <div className="topbar-device">
            <span className={`status-dot ${isHardwareConnected ? 'online' : ''}`} />
            <div>
              <div className="topbar-title">{displayControllerName}</div>
              <div className="topbar-meta">
                <span className={`chip ${isHardwareConnected ? 'success' : ''}`}>
                  {isHardwareConnected ? t('dualsense_connected') : t('dualsense_disconnected')}
                </span>
                {currentStatus.connected && (
                  <>
                    <span className="chip accent">
                      {currentStatus.connection_type === 'bluetooth' ? (
                        <Bluetooth size={11} />
                      ) : (
                        <Usb size={11} />
                      )}
                      {currentStatus.connection_type.toUpperCase()}
                    </span>
                    <span className="chip">
                      {currentStatus.battery.is_charging ? (
                        <BatteryCharging size={12} />
                      ) : (
                        <BatteryFull size={12} />
                      )}
                      {currentStatus.battery.percentage}%
                    </span>
                  </>
                )}
                <span className="chip">EMU: {currentStatus.emulation.mode.toUpperCase()}</span>
              </div>
            </div>
          </div>

          <div className="topbar-actions">
            {WebHidDualSenseController.isSupported() && (
              <button
                className={`btn sm ${isWebHidConnected ? 'primary' : ''}`}
                onClick={isWebHidConnected ? handleDisconnectWebHid : handleConnectWebHid}
              >
                <Zap size={13} />
                {isWebHidConnected ? 'WebHID ON' : 'WebHID'}
              </button>
            )}
            <button
              className={`btn sm ${currentStatus.mic_muted ? 'warn' : ''}`}
              onClick={handleToggleMic}
              title={t('btn_toggle_mic')}
            >
              {currentStatus.mic_muted ? <MicOff size={13} /> : <Mic size={13} />}
              {currentStatus.mic_muted ? 'MIC MUTED' : 'MIC'}
            </button>
            <button
              className={`btn sm ${currentStatus.udp_stats.running ? 'primary' : ''}`}
              onClick={() =>
                void postJson(
                  '/api/dualsense/udp/toggle',
                  { enabled: !currentStatus.udp_stats.running },
                  !currentStatus.udp_stats.running
                    ? t('toast_dualsense_udp_started')
                    : t('toast_dualsense_udp_stopped')
                )
              }
            >
              <Radio size={13} />
              UDP :{currentStatus.udp_stats.port}
            </button>
          </div>

          <div
            className="topbar-glow-line"
            style={{
              background: `linear-gradient(90deg, transparent, ${ambientColor}, transparent)`,
            }}
          />
        </header>

        {/* WORKSPACE VIEWPORT */}
        <main className="studio-viewport">
          <div className="studio-content" key={activeSubTab}>
            {!currentStatus.extension_enabled ? (
              <div className="card" style={{ textAlign: 'center', padding: '56px 24px' }}>
                <Power size={36} color="#64748b" style={{ marginBottom: '12px' }} />
                <h2 className="page-title" style={{ marginBottom: '8px' }}>
                  {lang === 'pt' ? 'Motor DualSense Studio Pausado' : 'DualSense Studio Engine Paused'}
                </h2>
                <p className="page-subtitle" style={{ maxWidth: '480px', margin: '0 auto 20px' }}>
                  {lang === 'pt'
                    ? 'Ative o motor para iniciar leitura HID, gatilhos adaptáveis, lightbar 40 Hz e controle virtual.'
                    : 'Start the engine to resume HID polling, adaptive triggers, 40 Hz lightbar, and virtual controller emulation.'}
                </p>
                <button
                  className="btn primary"
                  onClick={() =>
                    void postJson('/api/dualsense/extension/toggle', { enabled: true })
                  }
                >
                  <Power size={14} />
                  {lang === 'pt' ? 'Ativar Motor Agora' : 'Start Engine Now'}
                </button>
              </div>
            ) : (
              <>
                {activeSubTab === 'tester' && (
                  <ControllerTester
                    onShowToast={onShowToast}
                    isHardwareConnected={isHardwareConnected}
                    controllerName={displayControllerName}
                    onBackendRumble={() =>
                      void postJson(
                        '/api/dualsense/rumble',
                        { heavy: 180, light: 120, duration_ms: 400 },
                        t('toast_rumble_tested')
                      )
                    }
                  />
                )}
                {activeSubTab === 'triggers' && (
                  <TriggerStudio triggers={currentStatus.triggers} onApply={handleApplyTriggers} />
                )}
                {activeSubTab === 'lighting' && (
                  <LightingStudio lighting={currentStatus.lighting} onApply={handleApplyLighting} />
                )}
                {activeSubTab === 'mapping' && (
                  <MappingStudio
                    mapping={currentStatus.mapping}
                    cemuhookStats={currentStatus.cemuhook_stats}
                    onApplyMapping={(m: InputMappingConfig) =>
                      void postJson('/api/dualsense/mapping', m, t('toast_dualsense_mapping_applied'))
                    }
                    onToggleCemuhook={(enabled) =>
                      void postJson('/api/dualsense/cemuhook/toggle', { enabled })
                    }
                  />
                )}
                {activeSubTab === 'emulation' && (
                  <EmulationStudio
                    emulation={currentStatus.emulation}
                    virtualDriver={currentStatus.virtual_driver}
                    onApply={(e: EmulationConfig) =>
                      void postJson('/api/dualsense/emulation', e, t('toast_dualsense_emulation_applied'))
                    }
                  />
                )}
                {activeSubTab === 'haptics' && (
                  <HapticsAudioStudio
                    haptics={currentStatus.haptics}
                    metrics={currentStatus.haptics_metrics}
                    rumbleGain={currentStatus.rumble_gain}
                    connectionType={currentStatus.connection_type}
                    onApplyHaptics={(h: HapticsConfig) =>
                      void postJson('/api/dualsense/haptics', h, t('toast_dualsense_haptics_applied'))
                    }
                    onTestHdPulse={(freqHz, amplitude, durationMs) =>
                      void postJson(
                        '/api/dualsense/haptics/test',
                        { freq_hz: freqHz, amplitude, duration_ms: durationMs },
                        t('toast_rumble_tested')
                      )
                    }
                    onUpdateRumble={(payload) => void postJson('/api/dualsense/rumble', payload)}
                  />
                )}
                {activeSubTab === 'udp' && (
                  <UdpServerStudio
                    stats={currentStatus.udp_stats}
                    onToggle={(enabled) =>
                      void postJson(
                        '/api/dualsense/udp/toggle',
                        { enabled },
                        enabled ? t('toast_dualsense_udp_started') : t('toast_dualsense_udp_stopped')
                      )
                    }
                  />
                )}
                {activeSubTab === 'profiles' && (
                  <GameProfilesStudio
                    profiles={currentStatus.profiles}
                    currentTriggers={currentStatus.triggers}
                    currentLighting={currentStatus.lighting}
                    gameRules={currentStatus.game_profile_rules}
                    onApplyProfile={(id) =>
                      void postJson('/api/dualsense/profile', { id }, t('toast_dualsense_profile_applied'))
                    }
                    onSaveCustomProfile={(p: TriggerProfile) =>
                      void postJson('/api/dualsense/profiles/custom', p, t('toast_dualsense_profile_applied'))
                    }
                    onDeleteCustomProfile={(id) =>
                      void postJson('/api/dualsense/profiles/custom', { id }, undefined, 'DELETE')
                    }
                    onSaveGameRules={(rules: GameProfileRule[]) =>
                      void postJson('/api/dualsense/game-rules', { rules })
                    }
                  />
                )}
              </>
            )}
          </div>
        </main>
      </div>
    </div>
  );
};
