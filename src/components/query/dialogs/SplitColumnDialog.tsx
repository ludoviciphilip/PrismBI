import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls, selectCls } from './DialogShell';

export const SplitColumnDialog: React.FC<{
  column: string;
  onClose: () => void;
  onApply: (args: {
    mode: 'delimiterEach' | 'delimiterLeftmost' | 'delimiterRightmost' | 'positions' | 'fixedLength';
    delimiter?: string;
    positions?: number[];
    length?: number;
    into: string[];
  }) => void;
}> = ({ column, onClose, onApply }) => {
  const [mode, setMode] = useState<'delimiterEach' | 'delimiterLeftmost' | 'delimiterRightmost' | 'positions' | 'fixedLength'>('delimiterEach');
  const [delimiter, setDelimiter] = useState(',');
  const [length, setLength] = useState(1);
  const [positions, setPositions] = useState('0');
  const [intoCount, setIntoCount] = useState(2);

  const buildInto = () => Array.from({ length: intoCount }, (_, i) => `${column}.${i + 1}`);

  const handleApply = () => {
    if (mode === 'positions') {
      onApply({ mode, positions: positions.split(',').map(p => Number(p.trim())).filter(n => !Number.isNaN(n)), into: buildInto() });
    } else if (mode === 'fixedLength') {
      onApply({ mode, length, into: buildInto() });
    } else {
      onApply({ mode, delimiter, into: buildInto() });
    }
  };

  return (
    <DialogShell title="Dividir Coluna" subtitle={`Coluna: ${column}`} onClose={onClose} onConfirm={handleApply} confirmLabel="OK">
      <div>
        <FieldLabel>Modo</FieldLabel>
        <select value={mode} onChange={e => setMode(e.target.value as any)} className={`${selectCls} w-full`}>
          <option value="delimiterEach">Por delimitador — cada ocorrência</option>
          <option value="delimiterLeftmost">Por delimitador — mais à esquerda</option>
          <option value="delimiterRightmost">Por delimitador — mais à direita</option>
          <option value="positions">Por posições</option>
          <option value="fixedLength">Por comprimento fixo</option>
        </select>
      </div>
      {mode.startsWith('delimiter') && (
        <div>
          <FieldLabel>Delimitador</FieldLabel>
          <input value={delimiter} onChange={e => setDelimiter(e.target.value)} className={`${inputCls} w-full`} />
        </div>
      )}
      {mode === 'positions' && (
        <div>
          <FieldLabel>Posições (separadas por vírgula)</FieldLabel>
          <input value={positions} onChange={e => setPositions(e.target.value)} className={`${inputCls} w-full`} />
        </div>
      )}
      {mode === 'fixedLength' && (
        <div>
          <FieldLabel>Comprimento</FieldLabel>
          <input type="number" value={length} onChange={e => setLength(Number(e.target.value))} className={`${inputCls} w-full`} />
        </div>
      )}
      <div>
        <FieldLabel>Número de colunas resultantes</FieldLabel>
        <input type="number" min={2} value={intoCount} onChange={e => setIntoCount(Number(e.target.value))} className={`${inputCls} w-full`} />
      </div>
    </DialogShell>
  );
};
