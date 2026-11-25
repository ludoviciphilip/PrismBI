import React, { useState } from 'react';
import { DialogShell } from './DialogShell';

export const ChooseColumnsDialog: React.FC<{
  columns: string[];
  onClose: () => void;
  onApply: (columns: string[]) => void;
}> = ({ columns, onClose, onApply }) => {
  const [checked, setChecked] = useState<Set<string>>(new Set(columns));

  const toggle = (c: string) => {
    const next = new Set(checked);
    if (next.has(c)) next.delete(c);
    else next.add(c);
    setChecked(next);
  };

  return (
    <DialogShell title="Escolher Colunas" onClose={onClose} onConfirm={() => onApply(columns.filter(c => checked.has(c)))} confirmLabel="OK" widthClass="max-w-sm">
      <div className="flex items-center gap-2 mb-1">
        <button onClick={() => setChecked(new Set(columns))} className="text-[11px] text-amber-600 font-semibold">
          Selecionar tudo
        </button>
        <button onClick={() => setChecked(new Set())} className="text-[11px] text-amber-600 font-semibold">
          Limpar
        </button>
      </div>
      <div className="max-h-72 overflow-y-auto space-y-0.5">
        {columns.map(c => (
          <label key={c} data-testid={`choose-column-${c}`} className="flex items-center gap-2 px-1 py-1 hover:bg-slate-50 dark:hover:bg-slate-800 rounded">
            <input type="checkbox" checked={checked.has(c)} onChange={() => toggle(c)} />
            <span className="truncate">{c}</span>
          </label>
        ))}
      </div>
    </DialogShell>
  );
};
