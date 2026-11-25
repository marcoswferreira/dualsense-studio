import React, { useState, useEffect } from 'react';
import { Activity, Gamepad2, Plug, Unplug, Vibrate, Zap } from 'lucide-react';
import { useTranslation } from '../../i18n';
import {
  backendInputToLiveGamepad,
  BackendInputResponse,
  pollStandardGamepad,
  triggerHapticRumble,
  webHidController,
  LiveGamepadState,
  WebHidDualSenseController,
} from '../../utils/dualsenseHardware';

interface Props {
  onShowToast?: (message: string) => void;
  isHardwareConnected?: boolean;
  isBackendConnected?: boolean;
  controllerName?: string;
  onBackendRumble?: () => void;
}

const StickReticle: React.FC<{ label: string; x: number; y: number; clicked: boolean }> = ({
  label,
  x,
  y,
  clicked,
}) => {
  const cx = 56 + x * 42;
  const cy = 56 + y * 42;

  return (
    <div className="stick-unit">
      <span className="field-label" style={{ gap: '6px' }}>
        {label} {clicked && <span className="chip accent">CLICK</span>}
      </span>
      <svg role="img" width="112" height="112" viewBox="0 0 112 112" aria-label={label}>
        <circle
          cx="56"
          cy="56"
          r="50"
          fill="#0a0f18"
          stroke={clicked ? '#3b82f6' : 'rgba(148,163,184,0.2)'}
          strokeWidth={clicked ? 2.5 : 1.5}
        />
        <circle cx="56" cy="56" r="25" fill="none" stroke="rgba(148,163,184,0.09)" strokeDasharray="3 3" />
        <line x1="56" y1="8" x2="56" y2="104" stroke="rgba(148,163,184,0.12)" strokeWidth="1" />
        <line x1="8" y1="56" x2="104" y2="56" stroke="rgba(148,163,184,0.12)" strokeWidth="1" />
        <line x1="56" y1="56" x2={cx} y2={cy} stroke="rgba(59,130,246,0.65)" strokeWidth="2" strokeLinecap="round" />
        <circle cx={cx} cy={cy} r="9" fill="#3b82f6" stroke="#fff" strokeWidth="2" />
      </svg>
      <div className="stick-coords">
        X {x.toFixed(2)} · Y {y.toFixed(2)}
      </div>
    </div>
  );
};

export const ControllerTester: React.FC<Props> = ({
  onShowToast,
  isHardwareConnected = false,
  isBackendConnected = false,
  controllerName,
  onBackendRumble,
}) => {
  const { t, lang } = useTranslation();
  const [browserGamepad, setBrowserGamepad] = useState<LiveGamepadState | null>(null);
  const [backendGamepad, setBackendGamepad] = useState<LiveGamepadState | null>(null);
  const [isWebHidConnected, setIsWebHidConnected] = useState(webHidController.isConnected());
  const [webHidName, setWebHidName] = useState(webHidController.getDeviceName());
  const isWebHidAvailable = WebHidDualSenseController.isSupported();

  // Poll Browser Gamepad API via requestAnimationFrame
  useEffect(() => {
    let animId: number;
    const poll = () => {
      setBrowserGamepad(pollStandardGamepad());
      setIsWebHidConnected(webHidController.isConnected());
      setWebHidName(webHidController.getDeviceName());
      animId = requestAnimationFrame(poll);
    };
    animId = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(animId);
  }, []);

  // Poll live physical HID telemetry directly from Rust backend (/api/dualsense/input) at ~25 Hz
  useEffect(() => {
    let cancelled = false;
    let inFlight = false;

    const pollBackend = async () => {
      if (inFlight || cancelled) return;
      inFlight = true;
      try {
        const res = await fetch('/api/dualsense/input');
        if (res.ok && !cancelled) {
          const data: BackendInputResponse = await res.json();
          if (data.connected) {
            setBackendGamepad(backendInputToLiveGamepad(data));
          } else {
            setBackendGamepad(null);
          }
        }
      } catch {
        if (!cancelled) setBackendGamepad(null);
      } finally {
        inFlight = false;
      }
    };

    void pollBackend();
    const timer = window.setInterval(() => void pollBackend(), 40);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  const liveGamepad = backendGamepad ?? browserGamepad;

  const handleConnectWebHid = async () => {
    const ok = await webHidController.connect();
    if (ok) {
      setIsWebHidConnected(true);
      setWebHidName(webHidController.getDeviceName());
      onShowToast?.(t('toast_webhid_connected'));
    } else {
      onShowToast?.(t('toast_webhid_failed'));
    }
  };

  const handleDisconnectWebHid = async () => {
    await webHidController.disconnect();
    setIsWebHidConnected(false);
    onShowToast?.(t('toast_webhid_disconnected'));
  };

  const handleTestRumble = async () => {
    // Prefer direct Rust backend HID rumble when the physical DualSense is connected
    if (isBackendConnected && onBackendRumble) {
      onBackendRumble();
      return;
    }
    const ok = await triggerHapticRumble(450, 0.85, 0.85);
    if (ok) {
      onShowToast?.(t('toast_rumble_tested'));
    } else if (onBackendRumble) {
      onBackendRumble();
    } else {
      onShowToast?.(t('toast_rumble_failed'));
    }
  };

  const b = liveGamepad?.buttons;

  return (
    <>
      <div className="page-header">
        <div>
          <h1 className="page-title">{t('dualsense_tester_title')}</h1>
          <p className="page-subtitle">{t('dualsense_tester_subtitle')}</p>
        </div>
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
          {isWebHidAvailable &&
            (isWebHidConnected ? (
              <button className="btn danger" onClick={handleDisconnectWebHid}>
                <Unplug size={14} />
                {t('dualsense_disconnect_webhid')}
              </button>
            ) : (
              <button className="btn primary" onClick={handleConnectWebHid}>
                <Plug size={14} />
                {t('dualsense_connect_webhid')}
              </button>
            ))}
          <button className="btn" onClick={handleTestRumble}>
            <Vibrate size={14} />
            {t('dualsense_test_rumble')}
          </button>
        </div>
      </div>

      {/* Live Connection Telemetry Strip */}
      <div className="card" style={{ display: 'flex', flexWrap: 'wrap', gap: '10px', alignItems: 'center', justifyContent: 'space-between', padding: '12px 18px' }}>
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', alignItems: 'center' }}>
          <span className={`chip ${liveGamepad?.connected || isHardwareConnected ? 'success' : 'warn'}`}>
            <Activity size={12} />
            {backendGamepad?.connected
              ? `Live HID: ${backendGamepad.id}`
              : browserGamepad?.connected
                ? `Gamepad API: ${browserGamepad.id}`
                : isHardwareConnected
                  ? `${controllerName || 'DualSense'} (${lang === 'pt' ? 'Conectado' : 'Connected'})`
                  : t('dualsense_tester_no_gamepad')}
          </span>
          <span className={`chip ${isWebHidConnected ? 'accent' : ''}`}>
            <Zap size={12} />
            {isWebHidConnected
              ? `WebHID: ${webHidName}`
              : isWebHidAvailable
                ? t('dualsense_webhid_standby')
                : t('dualsense_webhid_unsupported')}
          </span>
        </div>
      </div>

      <div className="grid-2">
        {/* Analog Sticks & Triggers */}
        <div className="card">
          <div className="card-head">
            <span className="card-title">
              <Gamepad2 size={16} color="#60a5fa" />
              {t('dualsense_tester_analog_sticks')}
            </span>
          </div>

          <div className="stick-stage">
            <StickReticle
              label="Left Stick (L3)"
              x={liveGamepad?.leftStick.x ?? 0}
              y={liveGamepad?.leftStick.y ?? 0}
              clicked={Boolean(b?.l3)}
            />
            <StickReticle
              label="Right Stick (R3)"
              x={liveGamepad?.rightStick.x ?? 0}
              y={liveGamepad?.rightStick.y ?? 0}
              clicked={Boolean(b?.r3)}
            />
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', paddingTop: '14px', borderTop: '1px solid var(--border-subtle)' }}>
            <div className="field">
              <div className="field-label">
                <span>L2 Analog Trigger</span>
                <span className="field-val">{liveGamepad?.l2 ?? 0}%</span>
              </div>
              <div className="meter-track">
                <div className="meter-fill" style={{ width: `${liveGamepad?.l2 ?? 0}%` }} />
              </div>
            </div>

            <div className="field">
              <div className="field-label">
                <span>R2 Analog Trigger</span>
                <span className="field-val">{liveGamepad?.r2 ?? 0}%</span>
              </div>
              <div className="meter-track">
                <div className="meter-fill" style={{ width: `${liveGamepad?.r2 ?? 0}%` }} />
              </div>
            </div>
          </div>
        </div>

        {/* Face Buttons, D-Pad & System Keys */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'space-between' }}>
          <div className="card-head">
            <span className="card-title">
              <Activity size={16} color="#22d3ee" />
              {t('dualsense_tester_buttons')}
            </span>
          </div>

          <div className="pad-clusters">
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '8px' }}>
              <span className="field-label">D-PAD</span>
              <div className="cross-grid">
                <div />
                <div className={`pad-key ${b?.dpadUp ? 'active' : ''}`}>▲</div>
                <div />
                <div className={`pad-key ${b?.dpadLeft ? 'active' : ''}`}>◀</div>
                <div />
                <div className={`pad-key ${b?.dpadRight ? 'active' : ''}`}>▶</div>
                <div />
                <div className={`pad-key ${b?.dpadDown ? 'active' : ''}`}>▼</div>
                <div />
              </div>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '8px' }}>
              <span className="field-label">ACTION</span>
              <div className="cross-grid">
                <div />
                <div className={`pad-key round ps-triangle ${b?.triangle ? 'active' : ''}`}>△</div>
                <div />
                <div className={`pad-key round ps-square ${b?.square ? 'active' : ''}`}>□</div>
                <div />
                <div className={`pad-key round ps-circle ${b?.circle ? 'active' : ''}`}>○</div>
                <div />
                <div className={`pad-key round ps-cross ${b?.cross ? 'active' : ''}`}>✕</div>
                <div />
              </div>
            </div>
          </div>

          <div className="sys-keys">
            <span className={`chip ${b?.l1 ? 'accent' : ''}`}>L1</span>
            <span className={`chip ${b?.r1 ? 'accent' : ''}`}>R1</span>
            <span className={`chip ${b?.create ? 'accent' : ''}`}>CREATE</span>
            <span className={`chip ${b?.options ? 'accent' : ''}`}>OPTIONS</span>
            <span className={`chip ${b?.ps ? 'accent' : ''}`}>PS</span>
            <span className={`chip ${b?.touchpad ? 'accent' : ''}`}>TOUCHPAD</span>
          </div>
        </div>
      </div>
    </>
  );
};
