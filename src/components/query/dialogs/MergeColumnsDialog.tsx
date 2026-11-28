import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls } from './DialogShell';

export const MergeColumnsDialog: React.FC<{
  columns: string[];
  /** Pré-seleção vinda da grade, NA ORDEM DE SELEÇÃO (clique) — como o Power BI. */
  initialSelected?: string[];
  initialSeparator?: string;
  initialNewColumn?: string;
  onClose: () => void;
  onApply: (args: { columns: string[]; separator: string; newColumn: string }) => void;
}> = ({ columns, initialSelected, initialSeparator, initialNewColumn, onClose, onApply }) => {
  // Pré-marca as colunas já selecionadas na grade, preservando a ORDEM DE
  // CLIQUE: reabrir esta etapa pela engrenagem (editar) também usa esse
  // caminho, com `initialSelected` = `step.columns` (ordem salva na etapa).
  const [selected, setSelected] = useState<string[]>(initialSelected ?? []);
  const [separator, setSeparator] = useState(initialSeparator ?? ' - ');
  const [newColumn, setNewColumn] = useState(initialNewColumn ?? 'Mesclado');

  // Ctrl+clique nas checkboxes também acrescenta no FIM, igual à grade.
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
              {selected.includes(c) ? `${selected.indexOf(c) + 1}. ` : ''}
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
