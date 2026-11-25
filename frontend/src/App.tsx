import React, { useState, useCallback, useRef } from 'react';
import { CheckCircle2 } from 'lucide-react';
import { DualSenseTab } from './components/DualSenseTab/DualSenseTab';

export const App: React.FC = () => {
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
    }, 2800);
  }, []);

  return (
    <>
      <DualSenseTab onShowToast={showToast} />
      {toast && (
        <div role="status" className="studio-toast">
          <CheckCircle2 size={16} color="#60a5fa" />
          <span>{toast}</span>
        </div>
      )}
    </>
  );
};

export default App;
