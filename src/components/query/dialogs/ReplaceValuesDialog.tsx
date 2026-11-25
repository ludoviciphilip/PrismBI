import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls } from './DialogShell';

export const ReplaceValuesDialog: React.FC<{
  column: string;
  onClose: () => void;
  onApply: (args: { find: string; replace: string; matchEntireCell: boolean }) => void;
}> = ({ column, onClose, onApply }) => {
  const [find, setFind] = useState('');
  const [replace, setReplace] = useState('');
  const [matchEntireCell, setMatchEntireCell] = useState(true);

  return (
    <DialogShell
      title="Substituir Valores"
      subtitle={`Coluna: ${column}`}
      onClose={onClose}
      onConfirm={() => onApply({ find, replace, matchEntireCell })}
      confirmLabel="OK"
    >
      <div>
        <FieldLabel>Valor a localizar</FieldLabel>
        <input value={find} onChange={e => setFind(e.target.value)} className={`${inputCls} w-full`} />
      </div>
      <div>
        <FieldLabel>Substituir por</FieldLabel>
        <input value={replace} onChange={e => setReplace(e.target.value)} className={`${inputCls} w-full`} />
      </div>
      <label className="flex items-center gap-2">
        <input type="checkbox" checked={matchEntireCell} onChange={e => setMatchEntireCell(e.target.checked)} />
        Corresponder ao conteúdo inteiro da célula
      </label>
    </DialogShell>
  );
};
