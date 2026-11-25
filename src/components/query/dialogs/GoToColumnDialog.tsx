import React, { useState } from 'react';
import { DialogShell, inputCls } from './DialogShell';

/** "Ir para Coluna": lista as colunas e rola a grade até lá, como no Power Query. */
export const GoToColumnDialog: React.FC<{
  columns: string[];
  onClose: () => void;
  onSelect: (column: string) => void;
}> = ({ columns, onClose, onSelect }) => {
  const [search, setSearch] = useState('');
  const filtered = columns.filter(c => c.toLowerCase().includes(search.toLowerCase()));

  return (
    <DialogShell title="Ir para Coluna" onClose={onClose} widthClass="max-w-xs">
      <input placeholder="Pesquisar" value={search} onChange={e => setSearch(e.target.value)} className={`${inputCls} mb-2`} />
      <div className="max-h-72 overflow-y-auto space-y-0.5">
        {filtered.map(c => (
          <button
            key={c}
            data-testid={`goto-column-${c}`}
            onClick={() => {
              onSelect(c);
              onClose();
            }}
            className="w-full text-left px-2 py-1.5 rounded hover:bg-slate-50 dark:hover:bg-slate-800 truncate"
          >
            {c}
          </button>
        ))}
      </div>
    </DialogShell>
  );
};
