import React, { useState } from 'react';
import { DialogShell, FieldLabel } from './DialogShell';

export const AppendQueriesDialog: React.FC<{
  availableTables: { id: string; name: string }[];
  onClose: () => void;
  onApply: (queries: string[]) => void;
}> = ({ availableTables, onClose, onApply }) => {
  const [selected, setSelected] = useState<string[]>([]);
  const toggle = (name: string) => setSelected(s => (s.includes(name) ? s.filter(n => n !== name) : [...s, name]));

  return (
    <DialogShell title="Acrescentar Consultas" onClose={onClose} onConfirm={() => onApply(selected)} confirmLabel="OK" confirmDisabled={!selected.length}>
      <FieldLabel>Consultas a acrescentar</FieldLabel>
      <div className="border border-slate-200 dark:border-slate-700 rounded-lg max-h-48 overflow-y-auto">
        {availableTables.map(t => (
          <label key={t.id} className="flex items-center gap-2 px-2 py-1 hover:bg-slate-50 dark:hover:bg-slate-800">
            <input type="checkbox" checked={selected.includes(t.name)} onChange={() => toggle(t.name)} />
            {t.name}
          </label>
        ))}
      </div>
    </DialogShell>
  );
};
