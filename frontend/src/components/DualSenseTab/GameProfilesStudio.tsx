import React, { useState } from 'react';
import { FolderKanban, Play, Plus, Rocket, Save, Trash2, X } from 'lucide-react';
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
    onSaveGameRules([...gameRules, { process_name: proc, profile_id: ruleProfileId, enabled: true }]);
    setNewProc('');
  };

  return (
    <>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t('dualsense_profiles_title')}</h2>
          <p className="page-subtitle">{t('dualsense_profiles_subtitle')}</p>
        </div>
      </div>

      <div className="grid-auto">
        {profiles.map((profile) => {
          const isCustom = !BUILTIN_IDS.has(profile.id);
          return (
            <div key={profile.id} className="card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'space-between', gap: '12px' }}>
              <div>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '10px', marginBottom: '6px' }}>
                  <span style={{ fontSize: '14px', fontWeight: 600, color: '#fff' }}>{profile.name}</span>
                  <span
                    style={{
                      width: '14px',
                      height: '14px',
                      borderRadius: '50%',
                      flexShrink: 0,
                      background: `rgb(${profile.lightbar.r}, ${profile.lightbar.g}, ${profile.lightbar.b})`,
                      boxShadow: `0 0 8px rgba(${profile.lightbar.r}, ${profile.lightbar.g}, ${profile.lightbar.b}, 0.7)`,
                    }}
                  />
                </div>
                <p style={{ fontSize: '12.5px', color: 'var(--text-secondary)', lineHeight: 1.45 }}>{profile.description}</p>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                  <span className="chip">L2: {profile.triggers.left.mode.toUpperCase()}</span>
                  <span className="chip">R2: {profile.triggers.right.mode.toUpperCase()}</span>
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button className="btn primary" style={{ flex: 1 }} onClick={() => onApplyProfile(profile.id)}>
                    <Play size={13} />
                    {t('dualsense_apply_profile')}
                  </button>
                  {isCustom && (
                    <button className="btn danger" onClick={() => onDeleteCustomProfile(profile.id)} aria-label="Delete profile">
                      <Trash2 size={14} />
                    </button>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      <div className="grid-2">
        {/* Save Custom Profile */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div className="card-head" style={{ marginBottom: 0 }}>
            <span className="card-title">
              <Save size={15} color="#60a5fa" />
              {lang === 'pt' ? 'Salvar Setup Atual como Perfil' : 'Save Current Setup as Profile'}
            </span>
          </div>
          <input
            className="input"
            placeholder={lang === 'pt' ? 'Nome do perfil (ex.: Cyberpunk 2077)' : 'Profile name (e.g. Cyberpunk 2077)'}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
          />
          <input
            className="input"
            placeholder={lang === 'pt' ? 'Descrição opcional' : 'Optional description'}
            value={newDesc}
            onChange={(e) => setNewDesc(e.target.value)}
          />
          <button className="btn primary" onClick={handleCreateProfile} style={{ alignSelf: 'flex-start' }}>
            <Plus size={14} />
            {lang === 'pt' ? 'Salvar Perfil' : 'Save Profile'}
          </button>
        </div>

        {/* Auto-Switch Rules */}
        <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div className="card-head" style={{ marginBottom: 0 }}>
            <span className="card-title">
              <Rocket size={15} color="#22d3ee" />
              {lang === 'pt' ? 'Troca Automática por Executável' : 'Auto-Switch by Game Executable'}
            </span>
            <span className="chip"><FolderKanban size={11} /> {gameRules.length}</span>
          </div>

          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            <input
              className="input"
              style={{ flex: 1, minWidth: '160px' }}
              placeholder="Cyberpunk2077.exe / Returnal.exe"
              value={newProc}
              onChange={(e) => setNewProc(e.target.value)}
            />
            <select
              className="select"
              style={{ width: 'auto', minWidth: '150px' }}
              value={ruleProfileId}
              onChange={(e) => setRuleProfileId(e.target.value)}
            >
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
            <button className="btn primary" onClick={handleAddRule}>
              <Plus size={14} />
            </button>
          </div>

          {gameRules.length === 0 ? (
            <p className="page-subtitle" style={{ margin: 0 }}>
              {lang === 'pt' ? 'Nenhuma regra automática configurada.' : 'No automatic game rules configured.'}
            </p>
          ) : (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
              {gameRules.map((rule, idx) => (
                <span key={`${rule.process_name}-${idx}`} className="chip accent">
                  {rule.process_name} → {rule.profile_id}
                  <button
                    onClick={() => onSaveGameRules(gameRules.filter((_, i) => i !== idx))}
                    style={{ background: 'none', border: 'none', color: '#fca5a5', cursor: 'pointer', display: 'inline-flex' }}
                  >
                    <X size={12} />
                  </button>
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    </>
  );
};
