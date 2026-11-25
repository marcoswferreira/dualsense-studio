import React, { useState, useEffect } from 'react';
import { useTranslation } from '../../i18n';
import {
  ButtonAction,
  ButtonId,
  CemuhookStats,
  GyroActivation,
  InputMappingConfig,
  ResponseCurve,
  StickTuning,
  TouchpadMode,
  TriggerTuning,
} from '../../types/models';

interface Props {
  mapping: InputMappingConfig;
  cemuhookStats: CemuhookStats;
  onApplyMapping: (mapping: InputMappingConfig) => void;
  onToggleCemuhook: (enabled: boolean) => void;
}

const BUTTON_OPTIONS: { id: ButtonId; label: string }[] = [
  { id: 'cross', label: 'Cross (✖)' },
  { id: 'circle', label: 'Circle (●)' },
  { id: 'square', label: 'Square (◼)' },
  { id: 'triangle', label: 'Triangle (▲)' },
  { id: 'l1', label: 'L1' },
  { id: 'r1', label: 'R1' },
  { id: 'l3', label: 'L3 (Left Click)' },
  { id: 'r3', label: 'R3 (Right Click)' },
  { id: 'create', label: 'Create / Share' },
  { id: 'options', label: 'Options' },
  { id: 'touchpad_click', label: 'Touchpad Click' },
  { id: 'paddle_left', label: 'Edge Left Paddle' },
  { id: 'paddle_right', label: 'Edge Right Paddle' },
  { id: 'fn_left', label: 'Edge Fn Left' },
  { id: 'fn_right', label: 'Edge Fn Right' },
];

export const MappingStudio: React.FC<Props> = ({
  mapping: initialMapping,
  cemuhookStats,
  onApplyMapping,
  onToggleCemuhook,
}) => {
  const { t, lang } = useTranslation();
  const [mapping, setMapping] = useState<InputMappingConfig>(initialMapping);
  const [remapSource, setRemapSource] = useState<ButtonId>('paddle_left');
  const [remapTarget, setRemapTarget] = useState<ButtonId | 'disabled' | 'toggle_mic_mute'>('cross');

  useEffect(() => {
    setMapping(initialMapping);
  }, [initialMapping]);

  const commit = (updated: InputMappingConfig) => {
    setMapping(updated);
    onApplyMapping(updated);
  };

  const updateStick = (side: 'left_stick' | 'right_stick', patch: Partial<StickTuning>) => {
    commit({
      ...mapping,
      [side]: { ...mapping[side], ...patch },
    });
  };

  const updateTrigger = (side: 'left_trigger' | 'right_trigger', patch: Partial<TriggerTuning>) => {
    commit({
      ...mapping,
      [side]: { ...mapping[side], ...patch },
    });
  };

  const addRemap = () => {
    const action: ButtonAction =
      remapTarget === 'disabled'
        ? { kind: 'disabled' }
        : remapTarget === 'toggle_mic_mute'
          ? { kind: 'toggle_mic_mute' }
          : { kind: 'button', target: remapTarget };
    commit({
      ...mapping,
      button_remaps: {
        ...mapping.button_remaps,
        [remapSource]: action,
      },
    });
  };

  const removeRemap = (src: ButtonId) => {
    const next = { ...mapping.button_remaps };
    delete next[src];
    commit({ ...mapping, button_remaps: next });
  };

  const renderStickCard = (title: string, side: 'left_stick' | 'right_stick') => {
    const stick = mapping[side];
    return (
      <div className="deck-card">
        <h4>🕹️ {title}</h4>
        <div className="effect-sliders">
          <div className="slider-row">
            <label>Inner Deadzone: {Math.round(stick.inner_deadzone * 100)}%</label>
            <input
              type="range"
              min="0"
              max="0.45"
              step="0.01"
              value={stick.inner_deadzone}
              onChange={(e) => updateStick(side, { inner_deadzone: parseFloat(e.target.value) })}
            />
          </div>
          <div className="slider-row">
            <label>Outer Deadzone: {Math.round(stick.outer_deadzone * 100)}%</label>
            <input
              type="range"
              min="0.6"
              max="1.0"
              step="0.01"
              value={stick.outer_deadzone}
              onChange={(e) => updateStick(side, { outer_deadzone: parseFloat(e.target.value) })}
            />
          </div>
          <div className="slider-row">
            <label>Anti-Deadzone: {Math.round(stick.anti_deadzone * 100)}%</label>
            <input
              type="range"
              min="0"
              max="0.3"
              step="0.01"
              value={stick.anti_deadzone}
              onChange={(e) => updateStick(side, { anti_deadzone: parseFloat(e.target.value) })}
            />
          </div>
          <div className="form-group">
            <label>Response Curve</label>
            <select
              className="deck-select"
              value={stick.curve.type}
              onChange={(e) => {
                const kind = e.target.value;
                const curve: ResponseCurve =
                  kind === 'exponential'
                    ? { type: 'exponential', exponent: 1.6 }
                    : kind === 's_curve'
                      ? { type: 's_curve' }
                      : { type: 'linear' };
                updateStick(side, { curve });
              }}
            >
              <option value="linear">Linear</option>
              <option value="exponential">Exponential (Precision)</option>
              <option value="s_curve">S-Curve (Smooth)</option>
            </select>
          </div>
          <div className="checkbox-row">
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={stick.invert_x}
                onChange={(e) => updateStick(side, { invert_x: e.target.checked })}
              />
              Invert X
            </label>
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={stick.invert_y}
                onChange={(e) => updateStick(side, { invert_y: e.target.checked })}
              />
              Invert Y
            </label>
          </div>
        </div>
      </div>
    );
  };

  const activeRemaps = Object.entries(mapping.button_remaps) as [ButtonId, ButtonAction][];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      <div className="studio-header">
        <div>
          <h3>{t('dualsense_mapping_title')}</h3>
          <p className="section-subtitle">{t('dualsense_mapping_subtitle')}</p>
        </div>
      </div>

      {/* Analog Stick Deadzones & Curves */}
      <div className="triggers-grid">
        {renderStickCard(lang === 'pt' ? 'Analógico Esquerdo (L3)' : 'Left Stick (L3)', 'left_stick')}
        {renderStickCard(lang === 'pt' ? 'Analógico Direito (R3)' : 'Right Stick (R3)', 'right_stick')}
      </div>

      {/* Trigger Deadzones & Hair-Triggers */}
      <div className="triggers-grid">
        {(['left_trigger', 'right_trigger'] as const).map((side) => {
          const trig = mapping[side];
          const label = side === 'left_trigger' ? 'L2 Trigger Tuning' : 'R2 Trigger Tuning';
          return (
            <div key={side} className="deck-card">
              <h4>⚡ {label}</h4>
              <div className="effect-sliders">
                <div className="slider-row">
                  <label>Deadzone: {trig.deadzone}</label>
                  <input
                    type="range"
                    min="0"
                    max="120"
                    value={trig.deadzone}
                    onChange={(e) => updateTrigger(side, { deadzone: parseInt(e.target.value, 10) })}
                  />
                </div>
                <div className="slider-row">
                  <label>Hair-Trigger Max Threshold: {trig.max_threshold}</label>
                  <input
                    type="range"
                    min="40"
                    max="255"
                    value={trig.max_threshold}
                    onChange={(e) => updateTrigger(side, { max_threshold: parseInt(e.target.value, 10) })}
                  />
                </div>
                <div className="slider-row">
                  <label>Sensitivity: {trig.sensitivity.toFixed(2)}x</label>
                  <input
                    type="range"
                    min="0.5"
                    max="2.0"
                    step="0.05"
                    value={trig.sensitivity}
                    onChange={(e) => updateTrigger(side, { sensitivity: parseFloat(e.target.value) })}
                  />
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Button & Edge Paddle Remapping */}
      <div className="deck-card">
        <h4>🎛️ {lang === 'pt' ? 'Remapeamento de Botões & Paddles (DualSense Edge)' : 'Button & DualSense Edge Paddle Remapping'}</h4>
        <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap', alignItems: 'center', marginBottom: '12px' }}>
          <select className="deck-select" value={remapSource} onChange={(e) => setRemapSource(e.target.value as ButtonId)}>
            {BUTTON_OPTIONS.map((b) => (
              <option key={b.id} value={b.id}>{b.label}</option>
            ))}
          </select>
          <span>→</span>
          <select
            className="deck-select"
            value={remapTarget}
            onChange={(e) => setRemapTarget(e.target.value as ButtonId | 'disabled' | 'toggle_mic_mute')}
          >
            {BUTTON_OPTIONS.map((b) => (
              <option key={b.id} value={b.id}>{b.label}</option>
            ))}
            <option value="toggle_mic_mute">System: Toggle Mic Mute</option>
            <option value="disabled">Disabled</option>
          </select>
          <button className="deck-btn primary" onClick={addRemap}>
            + {lang === 'pt' ? 'Adicionar Remap' : 'Add Remap'}
          </button>
        </div>

        {activeRemaps.length === 0 ? (
          <p className="section-subtitle" style={{ margin: 0 }}>
            {lang === 'pt' ? 'Nenhum remapeamento ativo (layout padrão).' : 'No active button remaps (default layout).'}
          </p>
        ) : (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
            {activeRemaps.map(([src, act]) => (
              <span key={src} className="badge-tag" style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
                {src} → {act.kind === 'button' ? act.target : act.kind}
                <button
                  style={{ background: 'transparent', border: 'none', color: '#f87171', cursor: 'pointer', padding: 0 }}
                  onClick={() => removeRemap(src)}
                >
                  ✕
                </button>
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Touchpad & Motion / Cemuhook */}
      <div className="triggers-grid">
        <div className="deck-card">
          <h4>👆 {lang === 'pt' ? 'Ajustes do Touchpad' : 'Touchpad Settings'}</h4>
          <div className="form-group">
            <label>Touchpad Mode</label>
            <select
              className="deck-select"
              value={mapping.touchpad.mode}
              onChange={(e) =>
                commit({
                  ...mapping,
                  touchpad: { ...mapping.touchpad, mode: e.target.value as TouchpadMode },
                })
              }
            >
              <option value="passthrough">Passthrough (Native)</option>
              <option value="mouse">Mouse Cursor Control</option>
              <option value="split_buttons">Split Buttons (Left=Create / Right=Options)</option>
              <option value="disabled">Disabled</option>
            </select>
          </div>
          <div className="slider-row">
            <label>Sensitivity: {mapping.touchpad.sensitivity.toFixed(1)}x</label>
            <input
              type="range"
              min="0.2"
              max="3.0"
              step="0.1"
              value={mapping.touchpad.sensitivity}
              onChange={(e) =>
                commit({
                  ...mapping,
                  touchpad: { ...mapping.touchpad, sensitivity: parseFloat(e.target.value) },
                })
              }
            />
          </div>
          <div className="checkbox-row">
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={mapping.touchpad.tap_to_click}
                onChange={(e) =>
                  commit({
                    ...mapping,
                    touchpad: { ...mapping.touchpad, tap_to_click: e.target.checked },
                  })
                }
              />
              Tap to Click
            </label>
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={mapping.touchpad.two_finger_scroll}
                onChange={(e) =>
                  commit({
                    ...mapping,
                    touchpad: { ...mapping.touchpad, two_finger_scroll: e.target.checked },
                  })
                }
              />
              Two-Finger Scroll
            </label>
          </div>
        </div>

        <div className="deck-card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
            <h4 style={{ margin: 0 }}>🧭 Motion & Cemuhook UDP (:{cemuhookStats.port})</h4>
            <button
              className={`deck-btn ${cemuhookStats.running ? 'danger' : 'primary'}`}
              onClick={() => onToggleCemuhook(!cemuhookStats.running)}
            >
              {cemuhookStats.running ? 'Stop DSU' : 'Start DSU :26760'}
            </button>
          </div>
          <div className="form-group">
            <label>Gyro Activation Mode</label>
            <select
              className="deck-select"
              value={mapping.gyro_activation}
              onChange={(e) =>
                commit({ ...mapping, gyro_activation: e.target.value as GyroActivation })
              }
            >
              <option value="always">Always Active</option>
              <option value="hold_l2">Hold L2 (ADS Aim)</option>
              <option value="hold_r2">Hold R2</option>
              <option value="hold_touchpad">Touch Touchpad</option>
              <option value="toggle">Toggle</option>
            </select>
          </div>
          <div className="slider-row">
            <label>Gyro Smoothing: {Math.round(mapping.gyro_smoothing * 100)}%</label>
            <input
              type="range"
              min="0"
              max="0.95"
              step="0.05"
              value={mapping.gyro_smoothing}
              onChange={(e) => commit({ ...mapping, gyro_smoothing: parseFloat(e.target.value) })}
            />
          </div>
          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', marginTop: '10px' }}>
            <span className={`badge-tag ${cemuhookStats.running ? 'live-badge' : ''}`}>
              DSU: {cemuhookStats.running ? 'RUNNING' : 'STOPPED'}
            </span>
            <span className="badge-tag">Clients: {cemuhookStats.active_clients}</span>
            <span className="badge-tag">Packets Sent: {cemuhookStats.packets_sent}</span>
          </div>
        </div>
      </div>
    </div>
  );
};
