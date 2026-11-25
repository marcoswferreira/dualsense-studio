import React from 'react';
import { Activity, Radio, Server, Zap } from 'lucide-react';
import { useTranslation } from '../../i18n';
import { UdpServerStats } from '../../types/models';

interface Props {
  stats: UdpServerStats;
  onToggle: (enabled: boolean) => void;
}

export const UdpServerStudio: React.FC<Props> = ({ stats, onToggle }) => {
  const { t } = useTranslation();

  return (
    <>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t('dualsense_udp_title')}</h2>
          <p className="page-subtitle">{t('dualsense_udp_subtitle')}</p>
        </div>
        <button className={`btn ${stats.running ? 'danger' : 'primary'}`} onClick={() => onToggle(!stats.running)}>
          <Radio size={14} />
          {stats.running ? t('dualsense_udp_stop') : t('dualsense_udp_start')}
        </button>
      </div>

      <div className="grid-3">
        <div className="card">
          <div className="field-label" style={{ marginBottom: '8px' }}>
            <span>{t('dualsense_udp_status_label')}</span>
            <Server size={14} color="#60a5fa" />
          </div>
          <div style={{ fontSize: '20px', fontWeight: 700, color: stats.running ? '#4ade80' : '#f87171' }}>
            {stats.running ? t('dualsense_udp_active') : t('dualsense_udp_stopped')}
          </div>
          <div className="stick-coords" style={{ marginTop: '4px' }}>UDP Port :{stats.port}</div>
        </div>

        <div className="card">
          <div className="field-label" style={{ marginBottom: '8px' }}>
            <span>{t('dualsense_udp_packets')}</span>
            <Activity size={14} color="#22d3ee" />
          </div>
          <div style={{ fontSize: '20px', fontWeight: 700, color: '#fff' }}>
            {stats.packets_received.toLocaleString()}
          </div>
          <div className="stick-coords" style={{ marginTop: '4px' }}>{t('dualsense_udp_packets_sub')}</div>
        </div>

        <div className="card">
          <div className="field-label" style={{ marginBottom: '8px' }}>
            <span>{t('dualsense_udp_last_client')}</span>
            <Zap size={14} color="#fbbf24" />
          </div>
          <div style={{ fontSize: '16px', fontWeight: 600, fontFamily: 'var(--font-mono)', color: '#fff' }}>
            {stats.last_sender || '—'}
          </div>
          <div className="stick-coords" style={{ marginTop: '6px' }}>
            {stats.last_packet_time ? new Date(stats.last_packet_time).toLocaleTimeString() : t('dualsense_udp_no_traffic')}
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <span className="card-title">
            <Zap size={15} color="#60a5fa" />
            {t('dualsense_udp_compatible_title')}
          </span>
        </div>
        <p className="page-subtitle" style={{ marginBottom: '14px' }}>{t('dualsense_udp_compatible_desc')}</p>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
          {['Assetto Corsa', 'F1 23 / 24', 'Cyberpunk 2077', 'Spider-Man Remastered', 'Forza Horizon 5', 'DualSenseX Link V2'].map((name) => (
            <span key={name} className="chip accent">{name}</span>
          ))}
        </div>
      </div>
    </>
  );
};
