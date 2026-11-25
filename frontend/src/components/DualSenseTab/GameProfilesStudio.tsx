import React, { useState } from 'react';
import { useTranslation } from '../../i18n';
import {
  DualTriggerConfig,
  GameProfileRule,
  LightingConfig,
  TriggerProfile,
} from '../../types/models';

interface Props {
  profiles: TriggerProfile[];
  currentTriggers: DualTriggerConfig;
  currentLighting: LightingConfig;
  gameRules: GameProfileRule[];
  onApplyProfile: (profileId: string) => void;
  onSaveCustomProfile: (profile: TriggerProfile) => void;
  onDeleteCustomProfile: (profileId: string) => void;
  onSaveGameRules: (rules: GameProfileRule[]) => void;
}

const BUILTIN_IDS = new Set(['fps_tactical', 'racing_abs', 'flight_sim', 'ps_classic']);

export const GameProfilesStudio: React.FC<Props> = ({
  profiles,
  currentTriggers,
  currentLighting,
  gameRules,
  onApplyProfile,
  onSaveCustomProfile,
  onDeleteCustomProfile,
  onSaveGameRules,
}) => {
  const { t, lang } = useTranslation();
  const [newName, setNewName] = useState('');
  const [newDesc, setNewDesc] = useState('');
  const [newProc, setNewProc] = useState('');
  const [ruleProfileId, setRuleProfileId] = useState(profiles[0]?.id || 'fps_tactical');

  const handleCreateProfile = () => {
    const trimmed = newName.trim();
    if (!trimmed) return;
    const id = `custom_${trimmed.toLowerCase().replace(/[^a-z0-9]+/g, '_')}`;
    const color =
      currentLighting.lightbar.mode === 'static' || currentLighting.lightbar.mode === 'breathing'
        ? currentLighting.lightbar.color
        : { r: 0, g: 68, b: 204 };

    onSaveCustomProfile({
      id,
      name: trimmed,
      description: newDesc.trim() || 'Custom DualSense Studio trigger & RGB profile.',
      triggers: currentTriggers,
      lightbar: color,
    });
    setNewName('');
    setNewDesc('');
  };

  const handleAddRule = () => {
    const proc = newProc.trim();
    if (!proc) return;
    const updated = [...gameRules, { process_name: proc, profile_id: ruleProfileId, enabled: true }];
    onSaveGameRules(updated);
    setNewProc('');
  };

  const handleDeleteRule = (idx: number) => {
    const updated = gameRules.filter((_, i) => i !== idx);
    onSaveGameRules(updated);
  };

  return (
    <div className="profiles-studio-container" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      <div className="studio-header">
        <div>
          <h3>{t('dualsense_profiles_title')}</h3>
          <p className="section-subtitle">{t('dualsense_profiles_subtitle')}</p>
        </div>
      </div>

      <div className="profiles-grid">
        {profiles.map((profile) => {
          const isCustom = !BUILTIN_IDS.has(profile.id);
          return (
            <div key={profile.id} className="deck-card profile-card">
              <div className="profile-card-top">
                <h4>{profile.name}</h4>
                <div
                  className="profile-color-chip"
                  style={{
                    backgroundColor: `rgb(${profile.lightbar.r}, ${profile.lightbar.g}, ${profile.lightbar.b})`,
                  }}
                />
              </div>
              <p className="card-desc">{profile.description}</p>
              <div className="profile-details-preview">
                <span className="badge-tag">L2: {profile.triggers.left.mode.toUpperCase()}</span>
                <span className="badge-tag">R2: {profile.triggers.right.mode.toUpperCase()}</span>
              </div>
              <div style={{ display: 'flex', gap: '8px' }}>
                <button
                  className="deck-btn primary apply-profile-btn"
                  onClick={() => onApplyProfile(profile.id)}
                >
                  🎮 {t('dualsense_apply_profile')}
                </button>
                {isCustom && (
                  <button
                    className="deck-btn danger"
                    onClick={() => onDeleteCustomProfile(profile.id)}
                  >
                    🗑
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* Save Current Setup as Custom Profile */}
      <div className="deck-card">
        <h4>💾 {lang === 'pt' ? 'Salvar Configuração Atual como Perfil' : 'Save Current Triggers & RGB as Custom Profile'}</h4>
        <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap', alignItems: 'center' }}>
          <input
            type="text"
            className="deck-select"
            placeholder={lang === 'pt' ? 'Nome do perfil (ex.: Cyberpunk)' : 'Profile name (e.g. Cyberpunk)'}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            style={{ minWidth: '200px' }}
          />
          <input
            type="text"
            className="deck-select"
            placeholder={lang === 'pt' ? 'Descrição opcional' : 'Optional description'}
            value={newDesc}
            onChange={(e) => setNewDesc(e.target.value)}
            style={{ minWidth: '240px' }}
          />
          <button className="deck-btn primary" onClick={handleCreateProfile}>
            + {lang === 'pt' ? 'Salvar Perfil' : 'Save Profile'}
          </button>
        </div>
      </div>

      {/* Standalone Per-Game Auto-Switch Rules */}
      <div className="deck-card">
        <h4>🚀 {lang === 'pt' ? 'Troca Automática por Processo de Jogo (Standalone)' : 'Auto-Switch Profile by Game Executable (Standalone)'}</h4>
        <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap', alignItems: 'center', marginBottom: '12px' }}>
          <input
            type="text"
            className="deck-select"
            placeholder="Cyberpunk2077.exe / Returnal.exe"
            value={newProc}
            onChange={(e) => setNewProc(e.target.value)}
            style={{ minWidth: '240px' }}
          />
          <select
            className="deck-select"
            value={ruleProfileId}
            onChange={(e) => setRuleProfileId(e.target.value)}
          >
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
          <button className="deck-btn primary" onClick={handleAddRule}>
            + {lang === 'pt' ? 'Adicionar Regra' : 'Add Rule'}
          </button>
        </div>

        {gameRules.length === 0 ? (
          <p className="section-subtitle" style={{ margin: 0 }}>
            {lang === 'pt' ? 'Nenhuma regra automática configurada.' : 'No automatic game switching rules configured.'}
          </p>
        ) : (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
            {gameRules.map((rule, idx) => (
              <span key={`${rule.process_name}-${idx}`} className="badge-tag" style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
                {rule.process_name} → {rule.profile_id}
                <button
                  style={{ background: 'transparent', border: 'none', color: '#f87171', cursor: 'pointer', padding: 0 }}
                  onClick={() => handleDeleteRule(idx)}
                >
                  ✕
                </button>
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
