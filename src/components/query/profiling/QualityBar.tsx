import React from 'react';
import { ColumnQuality } from './quality';

/** Barra fina sob o cabeçalho: % Válido (verde) / Erro (vermelho) / Vazio (cinza). */
export const QualityBar: React.FC<{ quality: ColumnQuality }> = ({ quality }) => (
  <div data-testid="quality-bar" title={`${quality.validPct}% válido, ${quality.errorPct}% erro, ${quality.emptyPct}% vazio`} className="h-1.5 w-full flex overflow-hidden rounded-sm mt-1">
    <div className="bg-emerald-500 h-full" style={{ width: `${quality.validPct}%` }} />
    <div className="bg-red-500 h-full" style={{ width: `${quality.errorPct}%` }} />
    <div className="bg-slate-300 dark:bg-slate-600 h-full" style={{ width: `${quality.emptyPct}%` }} />
  </div>
);
