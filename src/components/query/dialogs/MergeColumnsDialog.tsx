import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls } from './DialogShell';

export const MergeColumnsDialog: React.FC<{
  columns: string[];
  onClose: () => void;
  onApply: (args: { columns: string[]; separator: string; newColumn: string }) => void;
}> = ({ columns, onClose, onApply }) => {
  const [selected, setSelected] = useState<string[]>([]);
  const [separator, setSeparator] = useState(' - ');
  const [newColumn, setNewColumn] = useState('Mesclado');

  const toggle = (c: string) => setSelected(s => (s.includes(c) ? s.filter(x => x !== c) : [...s, c]));

  return (
    <DialogShell
      title="Mesclar Colunas"
      onClose={onClose}
      onConfirm={() => onApply({ columns: selected, separator, newColumn })}
      confirmLabel="OK"
      confirmDisabled={selected.length < 2}
    >
      <div>
        <FieldLabel>Colunas a mesclar (na ordem de seleção)</FieldLabel>
        <div className="flex flex-wrap gap-2">
          {columns.map(c => (
            <label key={c} className="flex items-center gap-1 border border-slate-300 dark:border-slate-700 rounded-lg px-2 py-1">
              <input type="checkbox" checked={selected.includes(c)} onChange={() => toggle(c)} />
              {c}
            </label>
          ))}
        </div>
      </div>
      <div>
        <FieldLabel>Separador</FieldLabel>
        <input value={separator} onChange={e => setSeparator(e.target.value)} className={`${inputCls} w-full`} />
      </div>
      <div>
        <FieldLabel>Nome da nova coluna</FieldLabel>
        <input value={newColumn} onChange={e => setNewColumn(e.target.value)} className={`${inputCls} w-full`} />
      </div>
    </DialogShell>
  );
};
