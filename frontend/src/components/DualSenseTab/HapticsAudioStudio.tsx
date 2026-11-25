import React, { useState, useEffect } from 'react';
import { Square, Vibrate, Waves, Zap } from 'lucide-react';
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
    <>
      <div className="page-header">
        <div>
          <h1 className="page-title">{t('dualsense_haptics_title')}</h1>
          <p className="page-subtitle">{t('dualsense_haptics_subtitle')}</p>
        </div>
        <span className={`chip ${metrics.active ? 'success' : ''}`}>
          {connectionType.toUpperCase()} · {metrics.active ? `BT 0x32 @ ${metrics.sample_rate_hz} Hz` : 'STANDBY'}
        </span>
      </div>

      <div className="grid-2">
        {/* Bluetooth HD Haptics (Report 0x32) */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
          <div className="card-head" style={{ marginBottom: 0 }}>
            <span className="card-title">
              <Waves size={16} color="#22d3ee" />
              Bluetooth HD Haptics (0x32)
            </span>
            <button
              className={`btn sm ${haptics.enabled ? 'danger' : 'primary'}`}
              onClick={() =>
                commit({
                  ...haptics,
                  enabled: !haptics.enabled,
                  mode: !haptics.enabled && haptics.mode === 'legacy_rumble' ? 'bluetooth_hd_haptics' : haptics.mode,
                })
              }
            >
              {haptics.enabled ? 'Disable' : 'Enable'}
            </button>
          </div>

          <div className="field">
            <span className="field-label">Engine Mode</span>
            <select className="select" value={haptics.mode} onChange={(e) => commit({ ...haptics, mode: e.target.value as HapticsMode })}>
              <option value="bluetooth_hd_haptics">Bluetooth HD Haptics (Report 0x32 — 3 kHz S8)</option>
              <option value="audio_to_haptics">Audio-to-Haptics DSP (48 kHz → 3 kHz)</option>
              <option value="legacy_rumble">Standard Rumble Motors (0x02 / 0x31)</option>
              <option value="off">Off</option>
            </select>
          </div>

          <div className="field">
            <div className="field-label"><span>HD Haptics Gain</span><span className="field-val">{Math.round(haptics.gain * 100)}%</span></div>
            <input className="slider" type="range" min="0" max="2.0" step="0.05" value={haptics.gain} onChange={(e) => commit({ ...haptics, gain: parseFloat(e.target.value) })} />
          </div>

          <div className="field">
            <div className="field-label"><span>Low-Pass Filter Cutoff</span><span className="field-val">{Math.round(haptics.lowpass_cutoff_hz)} Hz</span></div>
            <input className="slider" type="range" min="40" max="500" step="10" value={haptics.lowpass_cutoff_hz} onChange={(e) => commit({ ...haptics, lowpass_cutoff_hz: parseFloat(e.target.value) })} />
          </div>

          <div className="grid-2" style={{ gap: '10px' }}>
            <div className="field">
              <div className="field-label"><span>Left Balance</span><span className="field-val">{Math.round(haptics.left_balance * 100)}%</span></div>
              <input className="slider" type="range" min="0" max="1.0" step="0.05" value={haptics.left_balance} onChange={(e) => commit({ ...haptics, left_balance: parseFloat(e.target.value) })} />
            </div>
            <div className="field">
              <div className="field-label"><span>Right Balance</span><span className="field-val">{Math.round(haptics.right_balance * 100)}%</span></div>
              <input className="slider" type="range" min="0" max="1.0" step="0.05" value={haptics.right_balance} onChange={(e) => commit({ ...haptics, right_balance: parseFloat(e.target.value) })} />
            </div>
          </div>

          <div style={{ display: 'flex', gap: '10px', alignItems: 'center', paddingTop: '10px', borderTop: '1px solid var(--border-subtle)', flexWrap: 'wrap' }}>
            <span className="field-val">{pulseFreq} Hz</span>
            <input className="slider" style={{ flex: 1, minWidth: '100px' }} type="range" min="40" max="300" step="10" value={pulseFreq} onChange={(e) => setPulseFreq(parseInt(e.target.value, 10))} />
            <button className="btn primary sm" onClick={() => onTestHdPulse(pulseFreq, 0.85, 300)}>
              <Zap size={13} />
              {lang === 'pt' ? 'Testar Pulso 0x32' : 'Test Pulse 0x32'}
            </button>
          </div>
        </div>

        {/* Standard Rumble Gain & Actuator Meters */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
          <div className="card-head" style={{ marginBottom: 0 }}>
            <span className="card-title">
              <Vibrate size={16} color="#60a5fa" />
              {lang === 'pt' ? 'Rumble Padrão (USB & Bluetooth)' : 'Standard Rumble Feedback'}
            </span>
            <span className="chip">0x32 Sent: {metrics.packets_sent.toLocaleString()}</span>
          </div>

          <p className="page-subtitle" style={{ margin: 0 }}>
            {lang === 'pt'
              ? 'Escala a intensidade de vibração vinda de jogos via ViGEmBus (XInput/DS4), Virtual DualSense e UDP.'
              : 'Scales vibration intensity from games via ViGEmBus (XInput/DS4), Virtual DualSense, and UDP.'}
          </p>

          <div className="field">
            <div className="field-label"><span>Global Rumble Intensity</span><span className="field-val">{Math.round(rumbleGain * 100)}%</span></div>
            <input
              className="slider"
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

          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            <button className="btn" onClick={() => onUpdateRumble({ heavy: 200, light: 120, duration_ms: 400 })}>
              <Vibrate size={14} />
              Test Heavy + Light
            </button>
            <button className="btn danger" onClick={() => onUpdateRumble({ heavy: 0, light: 0 })}>
              <Square size={12} />
              Stop Rumble
            </button>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginTop: 'auto', paddingTop: '12px', borderTop: '1px solid var(--border-subtle)' }}>
            <div className="field">
              <div className="field-label"><span>Left Actuator Level</span><span className="field-val">{Math.round(metrics.left_level * 100)}%</span></div>
              <div className="meter-track"><div className="meter-fill" style={{ width: `${Math.round(metrics.left_level * 100)}%` }} /></div>
            </div>
            <div className="field">
              <div className="field-label"><span>Right Actuator Level</span><span className="field-val">{Math.round(metrics.right_level * 100)}%</span></div>
              <div className="meter-track"><div className="meter-fill" style={{ width: `${Math.round(metrics.right_level * 100)}%` }} /></div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
};
