import React, { useState, useEffect } from 'react';
import { Copy, RotateCcw, Zap } from 'lucide-react';
import { useTranslation } from '../../i18n';
import { DualTriggerConfig, TriggerEffect, TriggerMode } from '../../types/models';

interface Props {
  triggers: DualTriggerConfig;
  onApply: (triggers: DualTriggerConfig) => void;
}

function estimateZones(effect: TriggerEffect): number[] {
  switch (effect.mode) {
    case 'off':
      return Array(10).fill(0);
    case 'feedback':
      return Array.from({ length: 10 }, (_, i) => (i >= effect.start_position ? (effect.force / 8) * 100 : 8));
    case 'weapon':
      return Array.from({ length: 10 }, (_, i) =>
        i >= effect.start_position && i <= effect.end_position ? (effect.force / 8) * 100 : 10
      );
    case 'bow':
      return Array.from({ length: 10 }, (_, i) =>
        i < effect.start_position
          ? 8
          : i <= effect.end_position
            ? Math.min(100, ((effect.force + ((i - effect.start_position) / Math.max(1, effect.end_position - effect.start_position)) * effect.snap_force) / 10) * 100)
            : 25
      );
    case 'machine_gun':
    case 'machine':
      return Array.from({ length: 10 }, (_, i) => (i >= effect.start_position && i <= effect.end_position ? (i % 2 === 0 ? 85 : 35) : 10));
    case 'galloping':
      return Array.from({ length: 10 }, (_, i) => (i === effect.first_foot || i === effect.second_foot ? 90 : 18));
    case 'vibration':
      return Array.from({ length: 10 }, (_, i) => (i >= effect.position ? (i % 2 === 0 ? 75 : 30) : 8));
    case 'game_cube':
      return [15, 15, 15, 15, 15, 20, 25, 95, 100, 100];
    case 'choppy':
      return [20, 75, 20, 75, 20, 75, 20, 75, 20, 75];
    case 'very_soft':
      return Array(10).fill(22);
    case 'soft':
      return Array(10).fill(38);
    case 'medium':
      return Array(10).fill(55);
    case 'hard':
      return Array(10).fill(72);
    case 'very_hard':
      return Array(10).fill(86);
    case 'hardest':
    case 'rigid':
      return Array(10).fill(100);
    case 'vibrate_trigger_pulse':
      return [20, 80, 35, 90, 35, 85, 35, 90, 35, 80];
    case 'custom_zone':
      return effect.zones.map((z) => Math.round((z / 255) * 100));
  }
}

export const TriggerStudio: React.FC<Props> = ({ triggers: initialTriggers, onApply }) => {
  const { t, lang } = useTranslation();
  const [triggers, setTriggers] = useState<DualTriggerConfig>(initialTriggers);

  useEffect(() => {
    setTriggers(initialTriggers);
  }, [initialTriggers]);

  const commit = (updated: DualTriggerConfig) => {
    setTriggers(updated);
    onApply(updated);
  };

  const updateSideMode = (side: 'left' | 'right', mode: TriggerMode) => {
    let newEffect: TriggerEffect = { mode: 'off' };
    switch (mode) {
      case 'feedback':
        newEffect = { mode: 'feedback', start_position: 2, force: 5 };
        break;
      case 'weapon':
        newEffect = { mode: 'weapon', start_position: 2, end_position: 6, force: 7 };
        break;
      case 'bow':
        newEffect = { mode: 'bow', start_position: 1, end_position: 8, force: 4, snap_force: 7 };
        break;
      case 'machine_gun':
        newEffect = { mode: 'machine_gun', start_position: 1, end_position: 8, strength: 7, frequency: 15 };
        break;
      case 'machine':
        newEffect = { mode: 'machine', start_position: 1, end_position: 9, strength_a: 4, strength_b: 7, frequency: 15, period: 2 };
        break;
      case 'galloping':
        newEffect = { mode: 'galloping', start_position: 0, end_position: 8, first_foot: 2, second_foot: 5, frequency: 10 };
        break;
      case 'vibration':
        newEffect = { mode: 'vibration', position: 3, amplitude: 8, frequency: 25 };
        break;
      case 'game_cube':
      case 'choppy':
      case 'very_soft':
      case 'soft':
      case 'medium':
      case 'hard':
      case 'very_hard':
      case 'hardest':
      case 'rigid':
      case 'vibrate_trigger_pulse':
        newEffect = { mode };
        break;
      case 'custom_zone':
        newEffect = { mode: 'custom_zone', zones: [0, 20, 50, 80, 120, 160, 200, 220, 240, 255] };
        break;
      case 'off':
      default:
        newEffect = { mode: 'off' };
        break;
    }
    commit({ ...triggers, [side]: newEffect });
  };

  const updateSideEffect = (side: 'left' | 'right', patch: Record<string, unknown>) => {
    const updatedEffect = { ...triggers[side], ...patch } as TriggerEffect;
    commit({ ...triggers, [side]: updatedEffect });
  };

  const renderPanel = (side: 'left' | 'right') => {
    const effect = triggers[side];
    const zones = estimateZones(effect);

    return (
      <div className="card">
        <div className="card-head">
          <span className="card-title">
            <Zap size={16} color="#60a5fa" />
            {side === 'left' ? `${t('dualsense_left_trigger')} (L2)` : `${t('dualsense_right_trigger')} (R2)`}
          </span>
          <span className={`chip ${effect.mode !== 'off' ? 'accent' : ''}`}>
            {effect.mode.toUpperCase()}
          </span>
        </div>

        {/* 10-Zone Force Profile Preview */}
        <div className="trigger-curve-bar" title="10-Zone Resistance Curve">
          {zones.map((h, i) => (
            <div
              key={i}
              className="trigger-zone-cell"
              style={{ height: `${Math.max(10, h)}%`, opacity: h > 0 ? 0.95 : 0.15 }}
            />
          ))}
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div className="field">
            <span className="field-label">{t('dualsense_trigger_mode')}</span>
            <select
              className="select"
              value={effect.mode}
              onChange={(e) => updateSideMode(side, e.target.value as TriggerMode)}
            >
              <optgroup label="Dynamic Mechanical Effects">
                <option value="off">{t('dualsense_mode_off')}</option>
                <option value="feedback">{t('dualsense_mode_feedback')}</option>
                <option value="weapon">{t('dualsense_mode_weapon')}</option>
                <option value="bow">{t('dualsense_mode_bow')}</option>
                <option value="machine_gun">{t('dualsense_mode_machinegun')}</option>
                <option value="machine">{t('dualsense_mode_machine')}</option>
                <option value="galloping">{t('dualsense_mode_galloping')}</option>
                <option value="vibration">{t('dualsense_mode_vibration')}</option>
                <option value="custom_zone">Custom 10-Zone Profile</option>
              </optgroup>
              <optgroup label="DualSenseX Presets">
                <option value="game_cube">GameCube (Analog + Digital Click)</option>
                <option value="choppy">Choppy (Segmented Steps)</option>
                <option value="very_soft">Very Soft</option>
                <option value="soft">Soft</option>
                <option value="medium">Medium</option>
                <option value="hard">Hard</option>
                <option value="very_hard">Very Hard</option>
                <option value="hardest">Hardest</option>
                <option value="rigid">Rigid Wall</option>
                <option value="vibrate_trigger_pulse">Vibrate Trigger Pulse</option>
              </optgroup>
            </select>
          </div>

          {effect.mode === 'feedback' && (
            <>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_start_pos')}</span><span className="field-val">{effect.start_position}</span></div>
                <input className="slider" type="range" min="0" max="9" value={effect.start_position} onChange={(e) => updateSideEffect(side, { start_position: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_force')}</span><span className="field-val">{effect.force} / 8</span></div>
                <input className="slider" type="range" min="0" max="8" value={effect.force} onChange={(e) => updateSideEffect(side, { force: parseInt(e.target.value, 10) })} />
              </div>
            </>
          )}

          {effect.mode === 'weapon' && (
            <>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_start_pos')}</span><span className="field-val">{effect.start_position}</span></div>
                <input className="slider" type="range" min="0" max="9" value={effect.start_position} onChange={(e) => updateSideEffect(side, { start_position: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_end_pos')}</span><span className="field-val">{effect.end_position}</span></div>
                <input className="slider" type="range" min="0" max="9" value={effect.end_position} onChange={(e) => updateSideEffect(side, { end_position: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_force')}</span><span className="field-val">{effect.force} / 8</span></div>
                <input className="slider" type="range" min="0" max="8" value={effect.force} onChange={(e) => updateSideEffect(side, { force: parseInt(e.target.value, 10) })} />
              </div>
            </>
          )}

          {effect.mode === 'bow' && (
            <>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_start_pos')}</span><span className="field-val">{effect.start_position}</span></div>
                <input className="slider" type="range" min="0" max="9" value={effect.start_position} onChange={(e) => updateSideEffect(side, { start_position: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_end_pos')}</span><span className="field-val">{effect.end_position}</span></div>
                <input className="slider" type="range" min="0" max="9" value={effect.end_position} onChange={(e) => updateSideEffect(side, { end_position: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_draw_force')}</span><span className="field-val">{effect.force} / 8</span></div>
                <input className="slider" type="range" min="0" max="8" value={effect.force} onChange={(e) => updateSideEffect(side, { force: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_snap_force')}</span><span className="field-val">{effect.snap_force} / 8</span></div>
                <input className="slider" type="range" min="0" max="8" value={effect.snap_force} onChange={(e) => updateSideEffect(side, { snap_force: parseInt(e.target.value, 10) })} />
              </div>
            </>
          )}

          {effect.mode === 'vibration' && (
            <>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_start_pos')}</span><span className="field-val">{effect.position}</span></div>
                <input className="slider" type="range" min="0" max="9" value={effect.position} onChange={(e) => updateSideEffect(side, { position: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_amplitude')}</span><span className="field-val">{effect.amplitude}</span></div>
                <input className="slider" type="range" min="0" max="63" value={effect.amplitude} onChange={(e) => updateSideEffect(side, { amplitude: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_frequency')}</span><span className="field-val">{effect.frequency} Hz</span></div>
                <input className="slider" type="range" min="1" max="40" value={effect.frequency} onChange={(e) => updateSideEffect(side, { frequency: parseInt(e.target.value, 10) })} />
              </div>
            </>
          )}

          {effect.mode === 'machine_gun' && (
            <>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_force')}</span><span className="field-val">{effect.strength} / 8</span></div>
                <input className="slider" type="range" min="0" max="8" value={effect.strength} onChange={(e) => updateSideEffect(side, { strength: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_frequency')}</span><span className="field-val">{effect.frequency} Hz</span></div>
                <input className="slider" type="range" min="1" max="30" value={effect.frequency} onChange={(e) => updateSideEffect(side, { frequency: parseInt(e.target.value, 10) })} />
              </div>
            </>
          )}

          {effect.mode === 'machine' && (
            <>
              <div className="field">
                <div className="field-label"><span>Strength A</span><span className="field-val">{effect.strength_a} / 7</span></div>
                <input className="slider" type="range" min="0" max="7" value={effect.strength_a} onChange={(e) => updateSideEffect(side, { strength_a: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>Strength B</span><span className="field-val">{effect.strength_b} / 7</span></div>
                <input className="slider" type="range" min="0" max="7" value={effect.strength_b} onChange={(e) => updateSideEffect(side, { strength_b: parseInt(e.target.value, 10) })} />
              </div>
              <div className="field">
                <div className="field-label"><span>{t('dualsense_frequency')}</span><span className="field-val">{effect.frequency} Hz</span></div>
                <input className="slider" type="range" min="1" max="40" value={effect.frequency} onChange={(e) => updateSideEffect(side, { frequency: parseInt(e.target.value, 10) })} />
              </div>
            </>
          )}
        </div>
      </div>
    );
  };

  return (
    <>
      <div className="page-header">
        <div>
          <h1 className="page-title">{t('dualsense_triggers_title')}</h1>
          <p className="page-subtitle">{t('dualsense_triggers_subtitle')}</p>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button className="btn sm" onClick={() => commit({ left: triggers.left, right: triggers.left })}>
            <Copy size={13} />
            {lang === 'pt' ? 'Espelhar L2 → R2' : 'Mirror L2 → R2'}
          </button>
          <button className="btn sm danger" onClick={() => commit({ left: { mode: 'off' }, right: { mode: 'off' } })}>
            <RotateCcw size={13} />
            {lang === 'pt' ? 'Resetar' : 'Reset'}
          </button>
        </div>
      </div>

      <div className="grid-2">
        {renderPanel('left')}
        {renderPanel('right')}
      </div>
    </>
  );
};
