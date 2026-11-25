import React, { useState, useEffect } from 'react';
import { Palette, Sparkles } from 'lucide-react';
import { useTranslation } from '../../i18n';
import { LightingConfig, MicLed, PlayerLed, RgbColor } from '../../types/models';

interface Props {
  lighting: LightingConfig;
  onApply: (lighting: LightingConfig) => void;
}

const COLOR_PRESETS: { label: string; color: RgbColor }[] = [
  { label: 'Sony Blue', color: { r: 0, g: 68, b: 204 } },
  { label: 'Cyberpunk Cyan', color: { r: 0, g: 240, b: 255 } },
  { label: 'Crimson Red', color: { r: 255, g: 24, b: 56 } },
  { label: 'Emerald', color: { r: 16, g: 220, b: 110 } },
  { label: 'Electric Purple', color: { r: 160, g: 32, b: 240 } },
  { label: 'Amber Gold', color: { r: 255, g: 145, b: 0 } },
  { label: 'Pure White', color: { r: 255, g: 255, b: 255 } },
  { label: 'Stealth Off', color: { r: 0, g: 0, b: 0 } },
];

const PLAYER_OPTIONS: { id: PlayerLed; label: string }[] = [
  { id: 'off', label: 'Off' },
  { id: 'player1', label: 'P1' },
  { id: 'player2', label: 'P2' },
  { id: 'player3', label: 'P3' },
  { id: 'player4', label: 'P4' },
  { id: 'all', label: 'All' },
];

const MIC_OPTIONS: { id: MicLed; label: string }[] = [
  { id: 'off', label: 'Off' },
  { id: 'solid', label: 'Solid' },
  { id: 'pulse', label: 'Pulse' },
];

export const LightingStudio: React.FC<Props> = ({ lighting: initialLighting, onApply }) => {
  const { t } = useTranslation();
  const [lighting, setLighting] = useState<LightingConfig>(initialLighting);

  useEffect(() => {
    setLighting(initialLighting);
  }, [initialLighting]);

  const commit = (updated: LightingConfig) => {
    setLighting(updated);
    onApply(updated);
  };

  const activeColor: RgbColor =
    lighting.lightbar.mode === 'static' || lighting.lightbar.mode === 'breathing'
      ? lighting.lightbar.color
      : lighting.lightbar.mode === 'off'
        ? { r: 20, g: 25, b: 35 }
        : { r: 59, g: 130, b: 246 };

  const hexString = `#${activeColor.r.toString(16).padStart(2, '0')}${activeColor.g.toString(16).padStart(2, '0')}${activeColor.b.toString(16).padStart(2, '0')}`;
  const animClass =
    lighting.lightbar.mode === 'breathing'
      ? 'anim-breathing'
      : lighting.lightbar.mode === 'rainbow'
        ? 'anim-rainbow'
        : '';

  const p = lighting.player_led;
  const dots = [
    p === 'player4' || p === 'all',
    p === 'player2' || p === 'player3' || p === 'player4' || p === 'all',
    p === 'player1' || p === 'player3' || p === 'all',
    p === 'player2' || p === 'player3' || p === 'player4' || p === 'all',
    p === 'player4' || p === 'all',
  ];

  return (
    <>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t('dualsense_lighting_title')}</h2>
          <p className="page-subtitle">{t('dualsense_lighting_subtitle')}</p>
        </div>
        <span className="chip accent">40 Hz HARDWARE SYNC</span>
      </div>

      <div className="grid-2">
        {/* Live Touchpad & Lightbar Preview */}
        <div className="card">
          <div className="card-head">
            <span className="card-title">
              <Sparkles size={16} color="#60a5fa" />
              {t('dualsense_lightbar_preview')}
            </span>
            <span className="chip">{lighting.lightbar.mode.toUpperCase()}</span>
          </div>

          <div className="lightbar-stage">
            <div
              className={`touchpad-shell ${animClass}`}
              style={{
                borderColor: `rgb(${activeColor.r}, ${activeColor.g}, ${activeColor.b})`,
                boxShadow:
                  lighting.lightbar.mode === 'off'
                    ? 'none'
                    : `0 0 28px 4px rgba(${activeColor.r}, ${activeColor.g}, ${activeColor.b}, 0.55), inset 0 0 18px rgba(${activeColor.r}, ${activeColor.g}, ${activeColor.b}, 0.18)`,
              }}
            >
              <span style={{ fontSize: '10px', fontFamily: 'var(--font-mono)', letterSpacing: '0.2em', color: 'var(--text-muted)' }}>
                DUALSENSE TOUCHPAD
              </span>
            </div>

            <div className="player-dots" title={`Player LED: ${lighting.player_led}`}>
              {dots.map((lit, idx) => (
                <span key={idx} className={`player-dot ${lit ? 'lit' : ''}`} />
              ))}
            </div>

            <span className={`chip ${lighting.mic_led !== 'off' ? 'warn' : ''}`}>
              MIC LED: {lighting.mic_led.toUpperCase()}
            </span>
          </div>
        </div>

        {/* Palette & LED Controls */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          <div className="card-head" style={{ marginBottom: 0 }}>
            <span className="card-title">
              <Palette size={16} color="#22d3ee" />
              {t('dualsense_lighting_palette')}
            </span>
          </div>

          <div className="field">
            <span className="field-label">{t('dualsense_lighting_mode')}</span>
            <select
              className="select"
              value={lighting.lightbar.mode}
              onChange={(e) => {
                const mode = e.target.value;
                if (mode === 'rainbow') commit({ ...lighting, lightbar: { mode: 'rainbow', speed: 1.0 } });
                else if (mode === 'battery_level') commit({ ...lighting, lightbar: { mode: 'battery_level' } });
                else if (mode === 'breathing') commit({ ...lighting, lightbar: { mode: 'breathing', color: activeColor, speed: 1.0 } });
                else if (mode === 'off') commit({ ...lighting, lightbar: { mode: 'off' } });
                else commit({ ...lighting, lightbar: { mode: 'static', color: activeColor } });
              }}
            >
              <option value="static">{t('dualsense_led_mode_static')}</option>
              <option value="breathing">{t('dualsense_led_mode_breathing')}</option>
              <option value="rainbow">{t('dualsense_led_mode_rainbow')}</option>
              <option value="battery_level">{t('dualsense_led_mode_battery')}</option>
              <option value="off">{t('dualsense_mode_off')}</option>
            </select>
          </div>

          <div className="swatch-grid">
            {COLOR_PRESETS.map((preset) => (
              <button
                key={preset.label}
                className="swatch"
                style={{ backgroundColor: `rgb(${preset.color.r}, ${preset.color.g}, ${preset.color.b})` }}
                onClick={() => commit({ ...lighting, lightbar: { mode: 'static', color: preset.color } })}
                title={preset.label}
              />
            ))}
          </div>

          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '12px' }}>
            <span className="field-label">{t('dualsense_custom_hex')}</span>
            <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
              <input
                type="color"
                value={hexString}
                onChange={(e) => {
                  const hex = e.target.value.replace('#', '');
                  const color = {
                    r: parseInt(hex.substring(0, 2), 16),
                    g: parseInt(hex.substring(2, 4), 16),
                    b: parseInt(hex.substring(4, 6), 16),
                  };
                  commit({ ...lighting, lightbar: { mode: 'static', color } });
                }}
                style={{ width: '38px', height: '30px', border: 'none', background: 'none', cursor: 'pointer' }}
              />
              <span className="chip">{hexString.toUpperCase()}</span>
            </div>
          </div>

          <div className="field">
            <span className="field-label">{t('dualsense_player_leds')}</span>
            <div className="segmented">
              {PLAYER_OPTIONS.map((opt) => (
                <button
                  key={opt.id}
                  className={`seg-btn ${lighting.player_led === opt.id ? 'active' : ''}`}
                  onClick={() => commit({ ...lighting, player_led: opt.id })}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>

          <div className="field">
            <span className="field-label">{t('dualsense_mic_led')}</span>
            <div className="segmented">
              {MIC_OPTIONS.map((opt) => (
                <button
                  key={opt.id}
                  className={`seg-btn ${lighting.mic_led === opt.id ? 'active' : ''}`}
                  onClick={() => commit({ ...lighting, mic_led: opt.id })}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>
    </>
  );
};
