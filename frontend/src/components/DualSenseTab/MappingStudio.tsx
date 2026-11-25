import React, { useState, useEffect } from 'react';
import { Compass, Plus, SlidersHorizontal, Touchpad, X, Zap } from 'lucide-react';
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
  { id: 'cross', label: 'Cross (✕)' },
  { id: 'circle', label: 'Circle (○)' },
  { id: 'square', label: 'Square (□)' },
  { id: 'triangle', label: 'Triangle (△)' },
  { id: 'l1', label: 'L1' },
  { id: 'r1', label: 'R1' },
  { id: 'l3', label: 'L3' },
  { id: 'r3', label: 'R3' },
  { id: 'create', label: 'Create' },
  { id: 'options', label: 'Options' },
  { id: 'touchpad_click', label: 'Touchpad Click' },
  { id: 'paddle_left', label: 'Edge Left Paddle' },
  { id: 'paddle_right', label: 'Edge Right Paddle' },
  { id: 'fn_left', label: 'Edge Fn Left' },
  { id: 'fn_right', label: 'Edge Fn Right' },
];

const DeadzoneRing: React.FC<{ stick: StickTuning }> = ({ stick }) => {
  const maxR = 40;
  const innerR = Math.max(2, stick.inner_deadzone * maxR);
  const outerR = Math.max(innerR + 2, stick.outer_deadzone * maxR);
  const antiR = Math.min(outerR, (stick.inner_deadzone + stick.anti_deadzone) * maxR);

  return (
    <svg width="92" height="92" viewBox="0 0 92 92" style={{ flexShrink: 0 }} aria-label="Deadzone Preview">
      <circle cx="46" cy="46" r={maxR} fill="#0a0f18" stroke="rgba(148,163,184,0.22)" strokeWidth="1.2" />
      <circle cx="46" cy="46" r={outerR} fill="rgba(59,130,246,0.1)" stroke="#3b82f6" strokeWidth="1.4" strokeDasharray="3 2" />
      {stick.anti_deadzone > 0 && (
        <circle cx="46" cy="46" r={antiR} fill="none" stroke="#22d3ee" strokeWidth="1.2" />
      )}
      <circle cx="46" cy="46" r={innerR} fill="rgba(239,68,68,0.28)" stroke="#ef4444" strokeWidth="1.2" />
      <line x1="46" y1="6" x2="46" y2="86" stroke="rgba(148,163,184,0.1)" />
      <line x1="6" y1="46" x2="86" y2="46" stroke="rgba(148,163,184,0.1)" />
    </svg>
  );
};

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

  const updateStick = (side: 'left_stick' | 'right_stick', patch: Partial<StickTuning>) =>
    commit({ ...mapping, [side]: { ...mapping[side], ...patch } });

  const updateTrigger = (side: 'left_trigger' | 'right_trigger', patch: Partial<TriggerTuning>) =>
    commit({ ...mapping, [side]: { ...mapping[side], ...patch } });

  const addRemap = () => {
    const action: ButtonAction =
      remapTarget === 'disabled'
        ? { kind: 'disabled' }
        : remapTarget === 'toggle_mic_mute'
          ? { kind: 'toggle_mic_mute' }
          : { kind: 'button', target: remapTarget };
    commit({ ...mapping, button_remaps: { ...mapping.button_remaps, [remapSource]: action } });
  };

  const removeRemap = (src: ButtonId) => {
    const next = { ...mapping.button_remaps };
    delete next[src];
    commit({ ...mapping, button_remaps: next });
  };

  const renderStickCard = (title: string, side: 'left_stick' | 'right_stick') => {
    const stick = mapping[side];
    return (
      <div className="card">
        <div className="card-head">
          <span className="card-title">
            <SlidersHorizontal size={15} color="#60a5fa" />
            {title}
          </span>
          <span className="chip">{stick.curve.type.toUpperCase()}</span>
        </div>

        <div style={{ display: 'flex', gap: '16px', alignItems: 'center' }}>
          <DeadzoneRing stick={stick} />
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '10px' }}>
            <div className="field">
              <div className="field-label"><span>Inner Deadzone</span><span className="field-val">{Math.round(stick.inner_deadzone * 100)}%</span></div>
              <input className="slider" type="range" min="0" max="0.45" step="0.01" value={stick.inner_deadzone} onChange={(e) => updateStick(side, { inner_deadzone: parseFloat(e.target.value) })} />
            </div>
            <div className="field">
              <div className="field-label"><span>Outer Saturation</span><span className="field-val">{Math.round(stick.outer_deadzone * 100)}%</span></div>
              <input className="slider" type="range" min="0.6" max="1.0" step="0.01" value={stick.outer_deadzone} onChange={(e) => updateStick(side, { outer_deadzone: parseFloat(e.target.value) })} />
            </div>
            <div className="field">
              <div className="field-label"><span>Anti-Deadzone</span><span className="field-val">{Math.round(stick.anti_deadzone * 100)}%</span></div>
              <input className="slider" type="range" min="0" max="0.3" step="0.01" value={stick.anti_deadzone} onChange={(e) => updateStick(side, { anti_deadzone: parseFloat(e.target.value) })} />
            </div>
          </div>
        </div>

        <div style={{ display: 'flex', gap: '12px', alignItems: 'center', justifyContent: 'space-between', marginTop: '14px', paddingTop: '12px', borderTop: '1px solid var(--border-subtle)', flexWrap: 'wrap' }}>
          <select
            className="select"
            style={{ width: 'auto', minWidth: '170px' }}
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
            <option value="linear">Curve: Linear</option>
            <option value="exponential">Curve: Exponential</option>
            <option value="s_curve">Curve: S-Curve</option>
          </select>

          <div className="check-row">
            <label className="check-label">
              <input type="checkbox" checked={stick.invert_x} onChange={(e) => updateStick(side, { invert_x: e.target.checked })} />
              Invert X
            </label>
            <label className="check-label">
              <input type="checkbox" checked={stick.invert_y} onChange={(e) => updateStick(side, { invert_y: e.target.checked })} />
              Invert Y
            </label>
          </div>
        </div>
      </div>
    );
  };

  const activeRemaps = Object.entries(mapping.button_remaps) as [ButtonId, ButtonAction][];

  return (
    <>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t('dualsense_mapping_title')}</h2>
          <p className="page-subtitle">{t('dualsense_mapping_subtitle')}</p>
        </div>
      </div>

      <div className="grid-2">
        {renderStickCard(lang === 'pt' ? 'Analógico Esquerdo (L3)' : 'Left Stick (L3)', 'left_stick')}
        {renderStickCard(lang === 'pt' ? 'Analógico Direito (R3)' : 'Right Stick (R3)', 'right_stick')}
      </div>

      {/* Hair-Trigger Tuning */}
      <div className="grid-2">
        {(['left_trigger', 'right_trigger'] as const).map((side) => {
          const trig = mapping[side];
          return (
            <div key={side} className="card" style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
              <div className="card-head" style={{ marginBottom: '4px' }}>
                <span className="card-title">
                  <Zap size={15} color="#22d3ee" />
                  {side === 'left_trigger' ? 'L2 Hair-Trigger & Deadzone' : 'R2 Hair-Trigger & Deadzone'}
                </span>
                <span className="chip">{trig.deadzone} → {trig.max_threshold}</span>
              </div>
              <div className="field">
                <div className="field-label"><span>Deadzone</span><span className="field-val">{trig.deadzone}</span></div>
                <input className="slider" type="range" min="0" max="120" value={trig.deadzone} onChange={(e) => updateTrigger(side, { deadzone: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>Max Threshold (Hair-Trigger)</span><span className="field-val">{trig.max_threshold}</span></div>
                <input className="slider" type="range" min="40" max="255" value={trig.max_threshold} onChange={(e) => updateTrigger(side, { max_threshold: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>Sensitivity</span><span className="field-val">{trig.sensitivity.toFixed(2)}x</span></div>
                <input className="slider" type="range" min="0.5" max="2.0" step="0.05" value={trig.sensitivity} onChange={(e) => updateTrigger(side, { sensitivity: parseFloat(e.target.value) })} />
              </div>
            </div>
          );
        })}
      </div>

      {/* Button & DualSense Edge Paddle Remapping */}
      <div className="card">
        <div className="card-head">
          <span className="card-title">
            <SlidersHorizontal size={15} color="#60a5fa" />
            {lang === 'pt' ? 'Remapeamento de Botões & Paddles (DualSense Edge)' : 'Button & DualSense Edge Paddle Remapping'}
          </span>
          <span className="chip">{activeRemaps.length} active</span>
        </div>

        <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap', alignItems: 'center', marginBottom: '12px' }}>
          <select className="select" style={{ width: 'auto', minWidth: '170px' }} value={remapSource} onChange={(e) => setRemapSource(e.target.value as ButtonId)}>
            {BUTTON_OPTIONS.map((b) => <option key={b.id} value={b.id}>{b.label}</option>)}
          </select>
          <span style={{ color: 'var(--text-muted)' }}>→</span>
          <select className="select" style={{ width: 'auto', minWidth: '190px' }} value={remapTarget} onChange={(e) => setRemapTarget(e.target.value as ButtonId | 'disabled' | 'toggle_mic_mute')}>
            {BUTTON_OPTIONS.map((b) => <option key={b.id} value={b.id}>{b.label}</option>)}
            <option value="toggle_mic_mute">System: Toggle Mic Mute</option>
            <option value="disabled">Disabled</option>
          </select>
          <button className="btn primary" onClick={addRemap}>
            <Plus size={14} />
            {lang === 'pt' ? 'Adicionar' : 'Add Remap'}
          </button>
        </div>

        {activeRemaps.length === 0 ? (
          <p className="page-subtitle" style={{ margin: 0 }}>
            {lang === 'pt' ? 'Layout padrão ativo — adicione remapeamentos acima.' : 'Standard layout active — add remaps above.'}
          </p>
        ) : (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
            {activeRemaps.map(([src, act]) => (
              <span key={src} className="chip accent">
                {src} → {act.kind === 'button' ? act.target : act.kind}
                <button
                  onClick={() => removeRemap(src)}
                  style={{ background: 'none', border: 'none', color: '#fca5a5', cursor: 'pointer', display: 'inline-flex' }}
                  aria-label={`Remove ${src}`}
                >
                  <X size={12} />
                </button>
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Touchpad + Motion / Cemuhook */}
      <div className="grid-2">
        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div className="card-head" style={{ marginBottom: 0 }}>
            <span className="card-title">
              <Touchpad size={15} color="#60a5fa" />
              {lang === 'pt' ? 'Touchpad' : 'Touchpad Settings'}
            </span>
            <span className="chip">{mapping.touchpad.mode.toUpperCase()}</span>
          </div>

          <div className="field">
            <span className="field-label">Mode</span>
            <select
              className="select"
              value={mapping.touchpad.mode}
              onChange={(e) =>
                commit({ ...mapping, touchpad: { ...mapping.touchpad, mode: e.target.value as TouchpadMode } })
              }
            >
              <option value="passthrough">Passthrough (Native)</option>
              <option value="mouse">Mouse Cursor Control</option>
              <option value="split_buttons">Split Buttons (Left=Create / Right=Options)</option>
              <option value="disabled">Disabled</option>
            </select>
          </div>

          <div className="field">
            <div className="field-label"><span>Sensitivity</span><span className="field-val">{mapping.touchpad.sensitivity.toFixed(1)}x</span></div>
            <input
              className="slider"
              type="range"
              min="0.2"
              max="3.0"
              step="0.1"
              value={mapping.touchpad.sensitivity}
              onChange={(e) =>
                commit({ ...mapping, touchpad: { ...mapping.touchpad, sensitivity: parseFloat(e.target.value) } })
              }
            />
          </div>

          <div className="check-row">
            <label className="check-label">
              <input type="checkbox" checked={mapping.touchpad.tap_to_click} onChange={(e) => commit({ ...mapping, touchpad: { ...mapping.touchpad, tap_to_click: e.target.checked } })} />
              Tap to Click
            </label>
            <label className="check-label">
              <input type="checkbox" checked={mapping.touchpad.two_finger_scroll} onChange={(e) => commit({ ...mapping, touchpad: { ...mapping.touchpad, two_finger_scroll: e.target.checked } })} />
              Two-Finger Scroll
            </label>
          </div>
        </div>

        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div className="card-head" style={{ marginBottom: 0 }}>
            <span className="card-title">
              <Compass size={15} color="#22d3ee" />
              Motion & Cemuhook UDP (:{cemuhookStats.port})
            </span>
            <button
              className={`btn sm ${cemuhookStats.running ? 'danger' : 'primary'}`}
              onClick={() => onToggleCemuhook(!cemuhookStats.running)}
            >
              {cemuhookStats.running ? 'Stop DSU' : 'Start DSU'}
            </button>
          </div>

          <div className="field">
            <span className="field-label">Gyro Activation</span>
            <select
              className="select"
              value={mapping.gyro_activation}
              onChange={(e) => commit({ ...mapping, gyro_activation: e.target.value as GyroActivation })}
            >
              <option value="always">Always Active</option>
              <option value="hold_l2">Hold L2 (ADS Aim)</option>
              <option value="hold_r2">Hold R2</option>
              <option value="hold_touchpad">Touch Touchpad</option>
              <option value="toggle">Toggle</option>
            </select>
          </div>

          <div className="field">
            <div className="field-label"><span>Gyro Smoothing</span><span className="field-val">{Math.round(mapping.gyro_smoothing * 100)}%</span></div>
            <input className="slider" type="range" min="0" max="0.95" step="0.05" value={mapping.gyro_smoothing} onChange={(e) => commit({ ...mapping, gyro_smoothing: parseFloat(e.target.value) })} />
          </div>

          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            <span className={`chip ${cemuhookStats.running ? 'success' : ''}`}>
              {cemuhookStats.running ? 'LISTENING' : 'STOPPED'}
            </span>
            <span className="chip">Clients: {cemuhookStats.active_clients}</span>
            <span className="chip">Sent: {cemuhookStats.packets_sent}</span>
          </div>
        </div>
      </div>
    </>
  );
};
