import React, { useState, useCallback, useRef } from 'react';
import { DualSenseTab } from './components/DualSenseTab/DualSenseTab';
import { useTranslation } from './i18n';

export const App: React.FC = () => {
  const { lang, toggleLang } = useTranslation();
  const [toast, setToast] = useState<string | null>(null);
  const timerRef = useRef<number | null>(null);

  const showToast = useCallback((message: string) => {
    setToast(message);
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
    }
    timerRef.current = window.setTimeout(() => {
      setToast(null);
      timerRef.current = null;
    }, 3000);
  }, []);

  return (
    <div style={{ minHeight: '100vh', padding: '20px 24px', maxWidth: '1280px', margin: '0 auto' }}>
      <header
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginBottom: '20px',
          paddingBottom: '12px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.08)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <span style={{ fontSize: '26px' }}>🎮</span>
          <div>
            <h1 style={{ margin: 0, fontSize: '20px', color: '#fff', fontWeight: 700 }}>
              DualSense Studio
            </h1>
            <p style={{ margin: 0, fontSize: '12px', color: 'var(--deck-text-muted, #9ca3af)' }}>
              {lang === 'pt'
                ? 'Central de Controle, Gatilhos Adaptáveis, Lightbar e Emulação'
                : 'Controller Hub, Adaptive Triggers, Lightbar & Emulation'}
            </p>
          </div>
        </div>

        <button className="deck-btn secondary" onClick={toggleLang}>
          🌐 {lang === 'pt' ? 'PT-BR' : 'EN'}
        </button>
      </header>

      <main>
        <DualSenseTab onShowToast={showToast} />
      </main>

      {toast && (
        <div
          role="status"
          style={{
            position: 'fixed',
            right: '24px',
            bottom: '24px',
            padding: '12px 18px',
            borderRadius: '10px',
            background: 'rgba(15, 23, 42, 0.95)',
            border: '1px solid rgba(59, 130, 246, 0.5)',
            color: '#f8fafc',
            fontSize: '13px',
            boxShadow: '0 8px 24px rgba(0, 0, 0, 0.45)',
            zIndex: 9999,
          }}
        >
          {toast}
        </div>
      )}
    </div>
  );
};

export default App;
