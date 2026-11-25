import React, { useState, useEffect } from 'react';
import { useTranslation } from '../../i18n';
import { DualTriggerConfig, TriggerEffect, TriggerMode } from '../../types/models';

interface Props {
  triggers: DualTriggerConfig;
  onApply: (triggers: DualTriggerConfig) => void;
}

export const TriggerStudio: React.FC<Props> = ({ triggers: initialTriggers, onApply }) => {
  const { t } = useTranslation();
  const [triggers, setTriggers] = useState<DualTriggerConfig>(initialTriggers);

  useEffect(() => {
    setTriggers(initialTriggers);
  }, [initialTriggers]);

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
        newEffect = { mode: 'game_cube' };
        break;
      case 'choppy':
        newEffect = { mode: 'choppy' };
        break;
      case 'very_soft':
        newEffect = { mode: 'very_soft' };
        break;
      case 'soft':
        newEffect = { mode: 'soft' };
        break;
      case 'medium':
        newEffect = { mode: 'medium' };
        break;
      case 'hard':
        newEffect = { mode: 'hard' };
        break;
      case 'very_hard':
        newEffect = { mode: 'very_hard' };
        break;
      case 'hardest':
        newEffect = { mode: 'hardest' };
        break;
      case 'rigid':
        newEffect = { mode: 'rigid' };
        break;
      case 'vibrate_trigger_pulse':
        newEffect = { mode: 'vibrate_trigger_pulse' };
        break;
      case 'custom_zone':
        newEffect = { mode: 'custom_zone', zones: [0, 20, 50, 80, 120, 160, 200, 220, 240, 255] };
        break;
      case 'off':
      default:
        newEffect = { mode: 'off' };
        break;
    }
    const updated: DualTriggerConfig = { ...triggers, [side]: newEffect };
    setTriggers(updated);
    onApply(updated);
  };

  const updateSideEffect = (side: 'left' | 'right', patch: Record<string, unknown>) => {
    const current = triggers[side];
    const updatedEffect = { ...current, ...patch } as TriggerEffect;
    const updated: DualTriggerConfig = { ...triggers, [side]: updatedEffect };
    setTriggers(updated);
    onApply(updated);
  };

  const renderEffectControls = (side: 'left' | 'right') => {
    const effect = triggers[side];

    return (
      <div className="trigger-effect-panel">
        <div className="form-group">
          <label>{t('dualsense_trigger_mode')}</label>
          <select
            className="deck-select"
            value={effect.mode}
            onChange={(e) => updateSideMode(side, e.target.value as TriggerMode)}
          >
            <option value="off">{t('dualsense_mode_off')}</option>
            <option value="feedback">{t('dualsense_mode_feedback')}</option>
            <option value="weapon">{t('dualsense_mode_weapon')}</option>
            <option value="bow">{t('dualsense_mode_bow')}</option>
            <option value="machine_gun">{t('dualsense_mode_machinegun')}</option>
            <option value="machine">{t('dualsense_mode_machine')}</option>
            <option value="galloping">{t('dualsense_mode_galloping')}</option>
            <option value="vibration">{t('dualsense_mode_vibration')}</option>
            <option value="game_cube">GameCube (Analog + Digital Click)</option>
            <option value="choppy">Choppy (Segmented Steps)</option>
            <option value="very_soft">Preset: Very Soft</option>
            <option value="soft">Preset: Soft</option>
            <option value="medium">Preset: Medium</option>
            <option value="hard">Preset: Hard</option>
            <option value="very_hard">Preset: Very Hard</option>
            <option value="hardest">Preset: Hardest</option>
            <option value="rigid">Preset: Rigid Wall</option>
            <option value="vibrate_trigger_pulse">Preset: Vibrate Trigger Pulse</option>
            <option value="custom_zone">Custom 10-Zone Profile</option>
          </select>
        </div>

        {effect.mode === 'feedback' && (
          <div className="effect-sliders">
            <div className="slider-row">
              <label>{t('dualsense_start_pos')}: {effect.start_position}</label>
              <input type="range" min="0" max="9" value={effect.start_position} onChange={(e) => updateSideEffect(side, { start_position: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_force')}: {effect.force}</label>
              <input type="range" min="0" max="8" value={effect.force} onChange={(e) => updateSideEffect(side, { force: parseInt(e.target.value, 10) })} />
            </div>
          </div>
        )}

        {effect.mode === 'weapon' && (
          <div className="effect-sliders">
            <div className="slider-row">
              <label>{t('dualsense_start_pos')}: {effect.start_position}</label>
              <input type="range" min="0" max="9" value={effect.start_position} onChange={(e) => updateSideEffect(side, { start_position: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_end_pos')}: {effect.end_position}</label>
              <input type="range" min="0" max="9" value={effect.end_position} onChange={(e) => updateSideEffect(side, { end_position: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_force')}: {effect.force}</label>
              <input type="range" min="0" max="8" value={effect.force} onChange={(e) => updateSideEffect(side, { force: parseInt(e.target.value, 10) })} />
            </div>
          </div>
        )}

        {effect.mode === 'bow' && (
          <div className="effect-sliders">
            <div className="slider-row">
              <label>{t('dualsense_start_pos')}: {effect.start_position}</label>
              <input type="range" min="0" max="9" value={effect.start_position} onChange={(e) => updateSideEffect(side, { start_position: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_end_pos')}: {effect.end_position}</label>
              <input type="range" min="0" max="9" value={effect.end_position} onChange={(e) => updateSideEffect(side, { end_position: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_draw_force')}: {effect.force}</label>
              <input type="range" min="0" max="8" value={effect.force} onChange={(e) => updateSideEffect(side, { force: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_snap_force')}: {effect.snap_force}</label>
              <input type="range" min="0" max="8" value={effect.snap_force} onChange={(e) => updateSideEffect(side, { snap_force: parseInt(e.target.value, 10) })} />
            </div>
          </div>
        )}

        {effect.mode === 'vibration' && (
          <div className="effect-sliders">
            <div className="slider-row">
              <label>{t('dualsense_start_pos')}: {effect.position}</label>
              <input type="range" min="0" max="9" value={effect.position} onChange={(e) => updateSideEffect(side, { position: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_amplitude')}: {effect.amplitude}</label>
              <input type="range" min="0" max="63" value={effect.amplitude} onChange={(e) => updateSideEffect(side, { amplitude: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_frequency')}: {effect.frequency} Hz</label>
              <input type="range" min="1" max="40" value={effect.frequency} onChange={(e) => updateSideEffect(side, { frequency: parseInt(e.target.value, 10) })} />
            </div>
          </div>
        )}

        {effect.mode === 'machine_gun' && (
          <div className="effect-sliders">
            <div className="slider-row">
              <label>{t('dualsense_force')}: {effect.strength}</label>
              <input type="range" min="0" max="8" value={effect.strength} onChange={(e) => updateSideEffect(side, { strength: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_frequency')}: {effect.frequency} Hz</label>
              <input type="range" min="1" max="30" value={effect.frequency} onChange={(e) => updateSideEffect(side, { frequency: parseInt(e.target.value, 10) })} />
            </div>
          </div>
        )}

        {effect.mode === 'machine' && (
          <div className="effect-sliders">
            <div className="slider-row">
              <label>Strength A: {effect.strength_a}</label>
              <input type="range" min="0" max="7" value={effect.strength_a} onChange={(e) => updateSideEffect(side, { strength_a: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>Strength B: {effect.strength_b}</label>
              <input type="range" min="0" max="7" value={effect.strength_b} onChange={(e) => updateSideEffect(side, { strength_b: parseInt(e.target.value, 10) })} />
            </div>
            <div className="slider-row">
              <label>{t('dualsense_frequency')}: {effect.frequency} Hz</label>
              <input type="range" min="1" max="40" value={effect.frequency} onChange={(e) => updateSideEffect(side, { frequency: parseInt(e.target.value, 10) })} />
            </div>
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="trigger-studio-container">
      <div className="studio-header">
        <div>
          <h3>{t('dualsense_triggers_title')}</h3>
          <p className="section-subtitle">{t('dualsense_triggers_subtitle')}</p>
        </div>
      </div>

      <div className="triggers-grid">
        <div className="deck-card trigger-card">
          <div className="trigger-card-header">
            <h4>🎮 {t('dualsense_left_trigger')} (L2)</h4>
            <span className="badge-tag">{triggers.left.mode.toUpperCase()}</span>
          </div>
          {renderEffectControls('left')}
        </div>

        <div className="deck-card trigger-card">
          <div className="trigger-card-header">
            <h4>🎮 {t('dualsense_right_trigger')} (R2)</h4>
            <span className="badge-tag">{triggers.right.mode.toUpperCase()}</span>
          </div>
          {renderEffectControls('right')}
        </div>
      </div>
    </div>
  );
};
