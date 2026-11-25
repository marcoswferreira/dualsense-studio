import React, { useState, useEffect } from 'react';
import { useTranslation } from '../../i18n';
import { HapticsConfig, HapticsMetrics, HapticsMode } from '../../types/models';

interface Props {
  haptics: HapticsConfig;
  metrics: HapticsMetrics;
  rumbleGain: number;
  connectionType: 'usb' | 'bluetooth';
  onApplyHaptics: (config: HapticsConfig) => void;
  onTestHdPulse: (freqHz: number, amplitude: number, durationMs: number) => void;
  onUpdateRumble: (payload: { heavy?: number; light?: number; gain?: number; duration_ms?: number }) => void;
}

export const HapticsAudioStudio: React.FC<Props> = ({
  haptics: initialHaptics,
  metrics,
  rumbleGain: initialGain,
  connectionType,
  onApplyHaptics,
  onTestHdPulse,
  onUpdateRumble,
}) => {
  const { t, lang } = useTranslation();
  const [haptics, setHaptics] = useState<HapticsConfig>(initialHaptics);
  const [rumbleGain, setRumbleGain] = useState<number>(initialGain);
  const [pulseFreq, setPulseFreq] = useState<number>(120);

  useEffect(() => {
    setHaptics(initialHaptics);
  }, [initialHaptics]);

  useEffect(() => {
    setRumbleGain(initialGain);
  }, [initialGain]);

  const commit = (updated: HapticsConfig) => {
    setHaptics(updated);
    onApplyHaptics(updated);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      <div className="studio-header">
        <div>
          <h3>{t('dualsense_haptics_title')}</h3>
          <p className="section-subtitle">{t('dualsense_haptics_subtitle')}</p>
        </div>
        <span className={`badge-tag ${metrics.active ? 'live-badge' : ''}`}>
          {connectionType.toUpperCase()} | {metrics.active ? `BT 0x32 @ ${metrics.sample_rate_hz} Hz` : 'STANDBY'}
        </span>
      </div>

      <div className="triggers-grid">
        {/* Bluetooth HD Haptics & Audio-to-Haptics */}
        <div className="deck-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
            <h4 style={{ margin: 0 }}>🌊 Bluetooth HD Haptics (Report 0x32)</h4>
            <button
              className={`deck-btn ${haptics.enabled ? 'danger' : 'primary'}`}
              onClick={() =>
                commit({
                  ...haptics,
                  enabled: !haptics.enabled,
                  mode: !haptics.enabled && haptics.mode === 'legacy_rumble' ? 'bluetooth_hd_haptics' : haptics.mode,
                })
              }
            >
              {haptics.enabled ? 'Disable HD Haptics' : 'Enable HD Haptics'}
            </button>
          </div>

          <div className="form-group">
            <label>Haptics Engine Mode</label>
            <select
              className="deck-select"
              value={haptics.mode}
              onChange={(e) => commit({ ...haptics, mode: e.target.value as HapticsMode })}
            >
              <option value="bluetooth_hd_haptics">Bluetooth HD Haptics (Report 0x32 — 3 kHz S8)</option>
              <option value="audio_to_haptics">Audio-to-Haptics DSP (48 kHz Loopback → 3 kHz)</option>
              <option value="legacy_rumble">Standard Rumble Motors (USB 0x02 / BT 0x31)</option>
              <option value="off">Off</option>
            </select>
          </div>

          <div className="effect-sliders">
            <div className="slider-row">
              <label>HD Haptics Gain: {Math.round(haptics.gain * 100)}%</label>
              <input
                type="range"
                min="0"
                max="2.0"
                step="0.05"
                value={haptics.gain}
                onChange={(e) => commit({ ...haptics, gain: parseFloat(e.target.value) })}
              />
            </div>

            <div className="slider-row">
              <label>Low-Pass Filter Cutoff: {Math.round(haptics.lowpass_cutoff_hz)} Hz</label>
              <input
                type="range"
                min="40"
                max="500"
                step="10"
                value={haptics.lowpass_cutoff_hz}
                onChange={(e) => commit({ ...haptics, lowpass_cutoff_hz: parseFloat(e.target.value) })}
              />
            </div>

            <div className="slider-row">
              <label>Left / Right Balance: L {Math.round(haptics.left_balance * 100)}% | R {Math.round(haptics.right_balance * 100)}%</label>
              <div style={{ display: 'flex', gap: '8px' }}>
                <input
                  type="range"
                  min="0"
                  max="1.0"
                  step="0.05"
                  value={haptics.left_balance}
                  onChange={(e) => commit({ ...haptics, left_balance: parseFloat(e.target.value) })}
                />
                <input
                  type="range"
                  min="0"
                  max="1.0"
                  step="0.05"
                  value={haptics.right_balance}
                  onChange={(e) => commit({ ...haptics, right_balance: parseFloat(e.target.value) })}
                />
              </div>
            </div>
          </div>

          <div style={{ display: 'flex', gap: '10px', alignItems: 'center', marginTop: '14px', flexWrap: 'wrap' }}>
            <label style={{ fontSize: '12px' }}>Pulse: {pulseFreq} Hz</label>
            <input
              type="range"
              min="40"
              max="300"
              step="10"
              value={pulseFreq}
              onChange={(e) => setPulseFreq(parseInt(e.target.value, 10))}
              style={{ width: '120px' }}
            />
            <button
              className="deck-btn primary"
              onClick={() => onTestHdPulse(pulseFreq, 0.85, 300)}
            >
              ⚡ {lang === 'pt' ? 'Testar Pulso HD (0x32)' : 'Test HD Pulse (0x32)'}
            </button>
          </div>
        </div>

        {/* Standard Rumble Gain & Live Telemetry */}
        <div className="deck-card">
          <h4>📳 {lang === 'pt' ? 'Rumble Padrão (USB & Bluetooth)' : 'Standard Rumble Feedback (USB & Bluetooth)'}</h4>
          <p className="section-subtitle">
            {lang === 'pt'
              ? 'Escala a intensidade de vibração recebida de jogos via ViGEmBus (XInput/DS4), Virtual DualSense e UDP.'
              : 'Scales vibration intensity received from games via ViGEmBus (XInput/DS4), Virtual DualSense, and UDP.'}
          </p>

          <div className="slider-row" style={{ marginBottom: '16px' }}>
            <label>Global Rumble Intensity: {Math.round(rumbleGain * 100)}%</label>
            <input
              type="range"
              min="0"
              max="2.0"
              step="0.05"
              value={rumbleGain}
              onChange={(e) => {
                const g = parseFloat(e.target.value);
                setRumbleGain(g);
                onUpdateRumble({ gain: g });
              }}
            />
          </div>

          <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap', marginBottom: '18px' }}>
            <button
              className="deck-btn secondary"
              onClick={() => onUpdateRumble({ heavy: 200, light: 120, duration_ms: 400 })}
            >
              📳 Test Heavy + Light Rumble
            </button>
            <button
              className="deck-btn danger"
              onClick={() => onUpdateRumble({ heavy: 0, light: 0 })}
            >
              ⏹ Stop Rumble
            </button>
          </div>

          {/* Live Haptics Telemetry */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            <div className="gauge-group">
              <div className="gauge-header">
                <span>Left Actuator Level</span>
                <span>{Math.round(metrics.left_level * 100)}%</span>
              </div>
              <div className="gauge-bar">
                <div className="gauge-fill" style={{ width: `${Math.round(metrics.left_level * 100)}%` }} />
              </div>
            </div>
            <div className="gauge-group">
              <div className="gauge-header">
                <span>Right Actuator Level</span>
                <span>{Math.round(metrics.right_level * 100)}%</span>
              </div>
              <div className="gauge-bar">
                <div className="gauge-fill" style={{ width: `${Math.round(metrics.right_level * 100)}%` }} />
              </div>
            </div>
            <span className="badge-tag" style={{ alignSelf: 'flex-start', marginTop: '6px' }}>
              BT 0x32 Packets Sent: {metrics.packets_sent.toLocaleString()}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};
