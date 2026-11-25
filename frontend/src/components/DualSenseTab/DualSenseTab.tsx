import React, { useState, useEffect, useCallback } from 'react';
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
  onRefreshAppStatus?: () => void;
}

export const DualSenseTab: React.FC<Props> = ({ onShowToast, onRefreshAppStatus }) => {
  const { t, lang } = useTranslation();
  const [activeSubTab, setActiveSubTab] = useState<SubTab>('tester');
  const [status, setStatus] = useState<DualSenseStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [isWebHidConnected, setIsWebHidConnected] = useState(webHidController.isConnected());

  useEffect(() => {
    const timer = setInterval(() => {
      setIsWebHidConnected(webHidController.isConnected());
    }, 800);
    return () => clearInterval(timer);
  }, []);

  const handleConnectWebHid = async () => {
    const ok = await webHidController.connect();
    setIsWebHidConnected(ok);
    if (ok) {
      onShowToast?.(t('toast_webhid_connected'));
      if (status) {
        await webHidController.sendReport(status.triggers, status.lighting);
      }
    } else {
      onShowToast?.(t('toast_webhid_failed'));
    }
  };

  const handleDisconnectWebHid = async () => {
    await webHidController.disconnect();
    setIsWebHidConnected(false);
    onShowToast?.(t('toast_webhid_disconnected'));
  };

  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch('/api/dualsense/status');
      if (res.ok) {
        const data: DualSenseStatus = await res.json();
        setStatus(data);
      }
    } catch (e) {
      console.error('Failed to load DualSense status', e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 2500);
    return () => clearInterval(interval);
  }, [fetchStatus]);

  const handleApplyTriggers = async (triggers: DualTriggerConfig) => {
    try {
      if (webHidController.isConnected()) {
        const currentLighting = status?.lighting || {
          lightbar: { mode: 'static' as const, color: { r: 0, g: 68, b: 204 } },
          player_led: 'player1' as const,
          mic_led: 'off' as const,
        };
        await webHidController.sendReport(triggers, currentLighting);
      }
      const res = await fetch('/api/dualsense/trigger', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(triggers),
      });
      if (res.ok) {
        onShowToast?.(t('toast_dualsense_triggers_applied'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleApplyLighting = async (lighting: LightingConfig) => {
    try {
      if (webHidController.isConnected()) {
        const currentTriggers = status?.triggers || {
          left: { mode: 'off' as const },
          right: { mode: 'off' as const },
        };
        await webHidController.sendReport(currentTriggers, lighting);
      }
      const res = await fetch('/api/dualsense/lightbar', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(lighting),
      });
      if (res.ok) {
        onShowToast?.(t('toast_dualsense_lighting_applied'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleApplyEmulation = async (emulation: EmulationConfig) => {
    try {
      const res = await fetch('/api/dualsense/emulation', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(emulation),
      });
      if (res.ok) {
        onShowToast?.(t('toast_dualsense_emulation_applied'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleApplyMapping = async (mapping: InputMappingConfig) => {
    try {
      const res = await fetch('/api/dualsense/mapping', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(mapping),
      });
      if (res.ok) {
        onShowToast?.(t('toast_dualsense_mapping_applied'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleApplyHaptics = async (haptics: HapticsConfig) => {
    try {
      const res = await fetch('/api/dualsense/haptics', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(haptics),
      });
      if (res.ok) {
        onShowToast?.(t('toast_dualsense_haptics_applied'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleTestHdPulse = async (freqHz: number, amplitude: number, durationMs: number) => {
    try {
      const res = await fetch('/api/dualsense/haptics/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ freq_hz: freqHz, amplitude, duration_ms: durationMs }),
      });
      if (res.ok) {
        onShowToast?.(t('toast_rumble_tested'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleUpdateRumble = async (payload: {
    heavy?: number;
    light?: number;
    gain?: number;
    duration_ms?: number;
  }) => {
    try {
      const res = await fetch('/api/dualsense/rumble', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (res.ok) {
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleToggleUdp = async (enabled: boolean) => {
    try {
      const res = await fetch('/api/dualsense/udp/toggle', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled }),
      });
      if (res.ok) {
        onShowToast?.(
          enabled ? t('toast_dualsense_udp_started') : t('toast_dualsense_udp_stopped')
        );
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleToggleCemuhook = async (enabled: boolean) => {
    try {
      const res = await fetch('/api/dualsense/cemuhook/toggle', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled }),
      });
      if (res.ok) {
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleToggleMic = async () => {
    try {
      const res = await fetch('/api/dualsense/mic/toggle', { method: 'POST' });
      if (res.ok) {
        const data = await res.json();
        onShowToast?.(data.mic_muted ? t('toast_mic_muted') : t('toast_mic_unmuted'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleApplyProfile = async (profileId: string) => {
    try {
      const res = await fetch('/api/dualsense/profile', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id: profileId }),
      });
      if (res.ok) {
        onShowToast?.(t('toast_dualsense_profile_applied'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleSaveCustomProfile = async (profile: TriggerProfile) => {
    try {
      const res = await fetch('/api/dualsense/profiles/custom', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(profile),
      });
      if (res.ok) {
        onShowToast?.(t('toast_dualsense_profile_applied'));
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleDeleteCustomProfile = async (profileId: string) => {
    try {
      const res = await fetch('/api/dualsense/profiles/custom', {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id: profileId }),
      });
      if (res.ok) {
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleSaveGameRules = async (rules: GameProfileRule[]) => {
    try {
      const res = await fetch('/api/dualsense/game-rules', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rules }),
      });
      if (res.ok) {
        fetchStatus();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleToggleExtension = async (enabled: boolean) => {
    try {
      const res = await fetch('/api/dualsense/extension/toggle', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled }),
      });
      if (res.ok) {
        await fetchStatus();
        onRefreshAppStatus?.();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  const handleToggleAutostart = async (autostart: boolean) => {
    try {
      const res = await fetch('/api/dualsense/extension/toggle', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ autostart }),
      });
      if (res.ok) {
        await fetchStatus();
        onRefreshAppStatus?.();
      }
    } catch {
      onShowToast?.(t('toast_error'));
    }
  };

  if (loading && !status) {
    return (
      <div className="tab-pane-content loading-pane">
        <p>{t('loading')}...</p>
      </div>
    );
  }

  const currentStatus: DualSenseStatus = status || {
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

  const isHardwareConnected = currentStatus.connected || isWebHidConnected;
  const displayControllerName = currentStatus.connected
    ? currentStatus.name
    : isWebHidConnected
      ? 'Sony DualSense Wireless Controller (WebHID)'
      : lang === 'pt'
        ? 'Nenhum Controle Conectado'
        : 'No Controller Connected';

  return (
    <div className="tab-pane-content dualsense-pane">
      {/* Standalone Service Control Header */}
      <div
        className="deck-card"
        style={{
          marginBottom: '16px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          flexWrap: 'wrap',
          gap: '16px',
          borderLeft: currentStatus.extension_enabled
            ? '4px solid #3b82f6'
            : '4px solid rgba(255, 255, 255, 0.2)',
        }}
      >
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '4px' }}>
            <h3 style={{ margin: 0, fontSize: '16px', color: '#fff' }}>
              🧩 DualSense Studio Engine
            </h3>
            <span className={`badge-tag ${currentStatus.extension_enabled ? 'live-badge' : ''}`}>
              {currentStatus.extension_enabled ? 'ACTIVE' : 'PAUSED'}
            </span>
          </div>
          <p style={{ margin: 0, fontSize: '13px', color: 'var(--deck-text-muted, #9ca3af)' }}>
            {lang === 'pt'
              ? 'Virtual DualSense, Hápticos BT 0x32, Gatilhos Adaptáveis, Lightbar 40 Hz, Remapeamento e Cemuhook.'
              : 'Virtual DualSense, BT 0x32 HD Haptics, Adaptive Triggers, 40 Hz Lightbar, Remapping & Cemuhook.'}
          </p>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '16px', flexWrap: 'wrap' }}>
          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px', color: '#e5e7eb', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={currentStatus.autostart_on_launch}
              onChange={(e) => handleToggleAutostart(e.target.checked)}
              style={{ width: '16px', height: '16px', accentColor: '#3b82f6' }}
            />
            {lang === 'pt' ? 'Iniciar automaticamente' : 'Autostart on launch'}
          </label>
          <button
            className={`deck-btn ${currentStatus.extension_enabled ? 'danger' : 'primary'}`}
            onClick={() => handleToggleExtension(!currentStatus.extension_enabled)}
          >
            {currentStatus.extension_enabled ? '⏻ Pause Engine' : '⚡ Start Engine'}
          </button>
        </div>
      </div>

      {currentStatus.extension_enabled && (
        <>
          {/* Controller Master Top Bar */}
          <div className="dualsense-master-bar deck-card">
            <div className="master-bar-left">
              <div className="controller-icon-badge" style={{ opacity: isHardwareConnected ? 1 : 0.45 }}>
                🎮
              </div>
              <div>
                <h2 className="master-title">{displayControllerName}</h2>
                <div className="master-subtitle">
                  <span className={`badge-tag ${isHardwareConnected ? 'live-badge' : ''}`}>
                    {isHardwareConnected ? t('dualsense_connected') : t('dualsense_disconnected')}
                  </span>
                  {currentStatus.connected && (
                    <>
                      <span className="badge-tag">{currentStatus.connection_type.toUpperCase()}</span>
                      <span className="badge-tag">
                        🔋 {currentStatus.battery.percentage}%{currentStatus.battery.is_charging ? ' ⚡' : ''}
                      </span>
                    </>
                  )}
                  <span className="badge-tag">EMU: {currentStatus.emulation.mode.toUpperCase()}</span>
                </div>
              </div>
            </div>

            <div className="master-bar-right">
              {WebHidDualSenseController.isSupported() && (
                <button
                  className={`deck-btn ${isWebHidConnected ? 'primary' : 'secondary'}`}
                  onClick={isWebHidConnected ? handleDisconnectWebHid : handleConnectWebHid}
                >
                  {isWebHidConnected ? '⚡ WebHID [ON]' : '⚡ WebHID'}
                </button>
              )}
              <button
                className={`deck-btn ${currentStatus.mic_muted ? 'danger' : 'secondary'}`}
                onClick={handleToggleMic}
              >
                {currentStatus.mic_muted ? '🎙️ MIC [MUTED]' : '🎙️ MIC [ACTIVE]'}
              </button>
              <button
                className="deck-btn secondary"
                onClick={() => handleToggleUdp(!currentStatus.udp_stats.running)}
              >
                {currentStatus.udp_stats.running
                  ? `📡 UDP :${currentStatus.udp_stats.port} [ON]`
                  : `📡 UDP :${currentStatus.udp_stats.port} [OFF]`}
              </button>
            </div>
          </div>

          {/* Sub-Navigation Pills */}
          <div className="dualsense-subnav">
            <button className={`subnav-pill ${activeSubTab === 'tester' ? 'active' : ''}`} onClick={() => setActiveSubTab('tester')}>
              🕹️ {t('dualsense_subtab_tester')}
            </button>
            <button className={`subnav-pill ${activeSubTab === 'triggers' ? 'active' : ''}`} onClick={() => setActiveSubTab('triggers')}>
              ⚡ {t('dualsense_subtab_triggers')}
            </button>
            <button className={`subnav-pill ${activeSubTab === 'lighting' ? 'active' : ''}`} onClick={() => setActiveSubTab('lighting')}>
              💡 {t('dualsense_subtab_lighting')}
            </button>
            <button className={`subnav-pill ${activeSubTab === 'mapping' ? 'active' : ''}`} onClick={() => setActiveSubTab('mapping')}>
              🎛️ {t('dualsense_subtab_mapping')}
            </button>
            <button className={`subnav-pill ${activeSubTab === 'emulation' ? 'active' : ''}`} onClick={() => setActiveSubTab('emulation')}>
              🎮 {t('dualsense_subtab_emulation')}
            </button>
            <button className={`subnav-pill ${activeSubTab === 'haptics' ? 'active' : ''}`} onClick={() => setActiveSubTab('haptics')}>
              🌊 {t('dualsense_subtab_haptics')}
            </button>
            <button className={`subnav-pill ${activeSubTab === 'udp' ? 'active' : ''}`} onClick={() => setActiveSubTab('udp')}>
              📡 {t('dualsense_subtab_udp')}
            </button>
            <button className={`subnav-pill ${activeSubTab === 'profiles' ? 'active' : ''}`} onClick={() => setActiveSubTab('profiles')}>
              📁 {t('dualsense_subtab_profiles')}
            </button>
          </div>

          {/* Active SubTab View */}
          <div className="dualsense-subview-area">
            {activeSubTab === 'tester' && (
              <ControllerTester
                onShowToast={onShowToast}
                isHardwareConnected={isHardwareConnected}
                controllerName={displayControllerName}
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
                onApplyMapping={handleApplyMapping}
                onToggleCemuhook={handleToggleCemuhook}
              />
            )}
            {activeSubTab === 'emulation' && (
              <EmulationStudio
                emulation={currentStatus.emulation}
                virtualDriver={currentStatus.virtual_driver}
                onApply={handleApplyEmulation}
              />
            )}
            {activeSubTab === 'haptics' && (
              <HapticsAudioStudio
                haptics={currentStatus.haptics}
                metrics={currentStatus.haptics_metrics}
                rumbleGain={currentStatus.rumble_gain}
                connectionType={currentStatus.connection_type}
                onApplyHaptics={handleApplyHaptics}
                onTestHdPulse={handleTestHdPulse}
                onUpdateRumble={handleUpdateRumble}
              />
            )}
            {activeSubTab === 'udp' && (
              <UdpServerStudio stats={currentStatus.udp_stats} onToggle={handleToggleUdp} />
            )}
            {activeSubTab === 'profiles' && (
              <GameProfilesStudio
                profiles={currentStatus.profiles}
                currentTriggers={currentStatus.triggers}
                currentLighting={currentStatus.lighting}
                gameRules={currentStatus.game_profile_rules}
                onApplyProfile={handleApplyProfile}
                onSaveCustomProfile={handleSaveCustomProfile}
                onDeleteCustomProfile={handleDeleteCustomProfile}
                onSaveGameRules={handleSaveGameRules}
              />
            )}
          </div>
        </>
      )}
    </div>
  );
};
