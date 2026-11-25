import React, { useState, useEffect } from 'react';
import { Compass, Cpu, Gamepad2, ShieldCheck, Zap } from 'lucide-react';
import { useTranslation } from '../../i18n';
import { EmulationConfig, EmulationMode, VirtualDualSenseDriverStatus } from '../../types/models';

interface Props {
  emulation: EmulationConfig;
  virtualDriver: VirtualDualSenseDriverStatus;
  onApply: (config: EmulationConfig) => void;
}

export const EmulationStudio: React.FC<Props> = ({
  emulation: initialEmulation,
  virtualDriver,
  onApply,
}) => {
  const { t } = useTranslation();
  const [emulation, setEmulation] = useState<EmulationConfig>(initialEmulation);

  useEffect(() => {
    setEmulation(initialEmulation);
  }, [initialEmulation]);

  const commit = (updated: EmulationConfig) => {
    setEmulation(updated);
    onApply(updated);
  };

  const modes: { id: EmulationMode; title: string; desc: string; icon: React.ReactNode }[] = [
    {
      id: 'dualsense',
      title: t('dualsense_mode_dualsense'),
      desc: t('dualsense_mode_dualsense_desc'),
      icon: <Zap size={20} color="#60a5fa" />,
    },
    {
      id: 'native',
      title: t('dualsense_mode_native'),
      desc: t('dualsense_mode_native_desc'),
      icon: <Gamepad2 size={20} color="#94a3b8" />,
    },
    {
      id: 'xbox360',
      title: t('dualsense_mode_xbox360'),
      desc: t('dualsense_mode_xbox360_desc'),
      icon: <Cpu size={20} color="#4ade80" />,
    },
    {
      id: 'dualshock4',
      title: t('dualsense_mode_ds4'),
      desc: t('dualsense_mode_ds4_desc'),
      icon: <Cpu size={20} color="#38bdf8" />,
    },
  ];

  return (
    <>
      <div className="page-header">
        <div>
          <h1 className="page-title">{t('dualsense_emulation_title')}</h1>
          <p className="page-subtitle">{t('dualsense_emulation_subtitle')}</p>
        </div>
      </div>

      {/* Driver Diagnostics Strip */}
      <div className="card" style={{ display: 'flex', flexWrap: 'wrap', gap: '10px', alignItems: 'center', justifyContent: 'space-between', padding: '12px 18px' }}>
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', alignItems: 'center' }}>
          <span className={`chip ${virtualDriver.available ? 'success' : 'warn'}`}>
            <ShieldCheck size={12} />
            Virtual DualSense ({virtualDriver.backend_name}): {virtualDriver.available ? 'READY' : 'STANDBY'}
          </span>
          <span className={`chip ${virtualDriver.vigem_installed ? 'success' : ''}`}>
            ViGEmBus: {virtualDriver.vigem_installed ? 'INSTALLED' : 'NOT DETECTED'}
          </span>
          <span className={`chip ${virtualDriver.hidhide_installed ? 'success' : ''}`}>
            HidHide: {virtualDriver.hidhide_installed ? 'ACTIVE' : 'NOT INSTALLED'}
          </span>
        </div>
        <span style={{ fontSize: '12px', color: 'var(--text-muted)' }}>{virtualDriver.detail}</span>
      </div>

      {/* 4 Balanced Mode Cards (native buttons for full keyboard & AT semantics) */}
      <div className="grid-4">
        {modes.map((m) => {
          const selected = emulation.mode === m.id;
          return (
            <button
              type="button"
              key={m.id}
              aria-pressed={selected}
              className={`card card-interactive ${selected ? 'selected' : ''}`}
              onClick={() => commit({ ...emulation, mode: m.id })}
              style={{
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'space-between',
                gap: '12px',
                textAlign: 'left',
                font: 'inherit',
                color: 'inherit',
              }}
            >
              <div>
                <div style={{ marginBottom: '10px' }} aria-hidden="true">{m.icon}</div>
                <div style={{ fontSize: '14px', fontWeight: 600, color: '#fff', marginBottom: '6px' }}>{m.title}</div>
                <p style={{ fontSize: '12.5px', color: 'var(--text-secondary)', lineHeight: 1.5 }}>{m.desc}</p>
              </div>
              <span className={`chip ${selected ? 'accent' : ''}`} style={{ alignSelf: 'flex-start' }}>
                {selected ? 'ACTIVE' : 'SELECT'}
              </span>
            </button>
          );
        })}
      </div>

      {/* Gyro Aiming Card */}
      <div className="card">
        <div className="card-head" style={{ marginBottom: emulation.gyro.enabled ? '16px' : 0 }}>
          <div>
            <span className="card-title">
              <Compass size={16} color="#22d3ee" />
              {t('dualsense_gyro_title')}
            </span>
            <p className="page-subtitle">{t('dualsense_gyro_subtitle')}</p>
          </div>
          <button
            className={`btn ${emulation.gyro.enabled ? 'danger' : 'primary'}`}
            onClick={() => commit({ ...emulation, gyro: { ...emulation.gyro, enabled: !emulation.gyro.enabled } })}
          >
            {emulation.gyro.enabled ? t('dualsense_gyro_disable') : t('dualsense_gyro_enable')}
          </button>
        </div>

        {emulation.gyro.enabled && (
          <div className="grid-2" style={{ paddingTop: '14px', borderTop: '1px solid var(--border-subtle)' }}>
            <div className="field">
              <div className="field-label"><span>{t('dualsense_gyro_sensitivity')}</span><span className="field-val">{emulation.gyro.sensitivity.toFixed(1)}x</span></div>
              <input className="slider" type="range" min="0.5" max="3.0" step="0.1" value={emulation.gyro.sensitivity} onChange={(e) => commit({ ...emulation, gyro: { ...emulation.gyro, sensitivity: parseFloat(e.target.value) } })} />
            </div>

            <div className="field">
              <div className="field-label"><span>{t('dualsense_gyro_deadzone')}</span><span className="field-val">{emulation.gyro.deadzone}</span></div>
              <input className="slider" type="range" min="0" max="150" step="5" value={emulation.gyro.deadzone} onChange={(e) => commit({ ...emulation, gyro: { ...emulation.gyro, deadzone: parseInt(e.target.value, 10) } })} />
            </div>

            <div className="check-row" style={{ gridColumn: '1 / -1' }}>
              <label className="check-label">
                <input type="checkbox" checked={emulation.gyro.invert_x} onChange={(e) => commit({ ...emulation, gyro: { ...emulation.gyro, invert_x: e.target.checked } })} />
                {t('dualsense_gyro_invert_x')}
              </label>
              <label className="check-label">
                <input type="checkbox" checked={emulation.gyro.invert_y} onChange={(e) => commit({ ...emulation, gyro: { ...emulation.gyro, invert_y: e.target.checked } })} />
                {t('dualsense_gyro_invert_y')}
              </label>
            </div>
          </div>
        )}
      </div>
    </>
  );
};
