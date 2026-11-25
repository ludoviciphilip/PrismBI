import React from 'react';
import { ColumnProfile, NUMERIC_TYPES } from './profile';
import { EtlType } from '../../../types/etl';

/** Painel inferior com as estatísticas completas da coluna ativa (como o "Perfil da Coluna" do Power Query). */
export const ProfilePanel: React.FC<{ column: string; type: EtlType | string; profile: ColumnProfile }> = ({ column, type, profile }) => {
  const isNumeric = NUMERIC_TYPES.includes(type as EtlType);
  const stat = (label: string, value: React.ReactNode) => (
    <div className="flex flex-col">
      <span className="text-slate-400 text-[10px] uppercase tracking-wide">{label}</span>
      <span className="font-semibold text-slate-700 dark:text-slate-200">{value ?? '—'}</span>
    </div>
  );

  return (
    <div data-testid="profile-panel" className="border-t border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 px-3 py-2 text-[11px]">
      <div className="flex items-center justify-between mb-2">
        <span className="font-bold text-slate-700 dark:text-slate-200">Perfil da coluna: {column}</span>
        <span className="text-[10px] text-slate-400">Perfil com base nas primeiras 200 linhas</span>
      </div>
      <div className="grid grid-cols-4 sm:grid-cols-6 gap-3 mb-3">
        {stat('Contagem', profile.count)}
        {stat('Erros', profile.errors)}
        {stat('Vazio', profile.empty)}
        {stat('Distintos', profile.distinct)}
        {stat('Únicos', profile.unique)}
        {isNumeric ? (
          <>
            {stat('Mínimo', profile.min)}
            {stat('Máximo', profile.max)}
            {stat('Média', profile.mean)}
            {stat('Desvio Padrão', profile.stdDev)}
          </>
        ) : (
          <>
            {stat('Strings Vazias', profile.emptyStrings)}
            {stat('Comprimento Mín.', profile.minLength)}
            {stat('Comprimento Máx.', profile.maxLength)}
          </>
        )}
      </div>
      <div className="flex items-end gap-0.5 h-16">
        {profile.distribution.buckets.map(b => (
          <div key={b.value} title={`${b.value}: ${b.count} (${b.pct}%)`} className="flex-1 bg-sky-400 dark:bg-sky-600 min-w-[4px]" style={{ height: `${b.pct}%` }} />
        ))}
      </div>
    </div>
  );
};
