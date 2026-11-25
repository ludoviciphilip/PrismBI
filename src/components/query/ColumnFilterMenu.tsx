import React, { useMemo, useState } from 'react';
import { Cond, Scalar } from '../../types/etl';
import { inputCls } from './dialogs/DialogShell';

export const ColumnFilterMenu: React.FC<{
  column: string;
  rows: Record<string, any>[];
  onClose: () => void;
  onApply: (condition: Cond) => void;
}> = ({ column, rows, onClose, onApply }) => {
  const [search, setSearch] = useState('');
  const distinctValues = useMemo(() => {
    const set = new Set<string>();
    rows.forEach(r => set.add(String(r[column] ?? '')));
    return Array.from(set).sort();
  }, [rows, column]);

  const [checked, setChecked] = useState<Set<string>>(new Set(distinctValues));
  const [mode, setMode] = useState<'list' | 'text' | 'number'>('list');
  const [textOp, setTextOp] = useState<'contains' | 'eq' | 'startsWith' | 'endsWith'>('contains');
  const [textValue, setTextValue] = useState('');
  const [numOp, setNumOp] = useState<'eq' | 'gt' | 'gte' | 'lt' | 'lte'>('eq');
  const [numValue, setNumValue] = useState('');

  const toggle = (v: string) => {
    const next = new Set(checked);
    if (next.has(v)) next.delete(v);
    else next.add(v);
    setChecked(next);
  };

  const filteredList = distinctValues.filter(v => v.toLowerCase().includes(search.toLowerCase()));

  const apply = () => {
    if (mode === 'list') {
      const values: Scalar[] = Array.from(checked);
      onApply({ column, op: 'in', values });
    } else if (mode === 'text') {
      onApply({ column, op: textOp, value: textValue });
    } else {
      onApply({ column, op: numOp, value: Number(numValue) });
    }
    onClose();
  };

  return (
    <div
      data-testid={`column-filter-${column}`}
      className="absolute z-40 mt-1 w-64 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-xl p-2 text-[11px]"
    >
      <div className="flex gap-1 mb-2">
        {(['list', 'text', 'number'] as const).map(m => (
          <button
            key={m}
            onClick={() => setMode(m)}
            className={`px-2 py-0.5 rounded-md font-semibold ${mode === m ? 'bg-amber-500 text-slate-950' : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300'}`}
          >
            {m === 'list' ? 'Lista' : m === 'text' ? 'Texto' : 'Número'}
          </button>
        ))}
      </div>

      {mode === 'list' && (
        <>
          <input
            placeholder="Pesquisar"
            value={search}
            onChange={e => setSearch(e.target.value)}
            className={`${inputCls} mb-2 text-[11px]`}
          />
          <div className="max-h-48 overflow-y-auto space-y-0.5">
            {filteredList.map(v => (
              <label key={v} className="flex items-center gap-1.5 px-1 py-0.5 hover:bg-slate-50 dark:hover:bg-slate-800 rounded">
                <input type="checkbox" checked={checked.has(v)} onChange={() => toggle(v)} />
                <span className="truncate">{v || '(vazio)'}</span>
              </label>
            ))}
          </div>
        </>
      )}

      {mode === 'text' && (
        <div className="space-y-1">
          <select value={textOp} onChange={e => setTextOp(e.target.value as any)} className={`${inputCls} text-[11px]`}>
            <option value="contains">Contém</option>
            <option value="eq">É igual a</option>
            <option value="startsWith">Começa com</option>
            <option value="endsWith">Termina com</option>
          </select>
          <input value={textValue} onChange={e => setTextValue(e.target.value)} className={`${inputCls} text-[11px]`} />
        </div>
      )}

      {mode === 'number' && (
        <div className="space-y-1">
          <select value={numOp} onChange={e => setNumOp(e.target.value as any)} className={`${inputCls} text-[11px]`}>
            <option value="eq">É igual a</option>
            <option value="gt">Maior que</option>
            <option value="gte">Maior ou igual a</option>
            <option value="lt">Menor que</option>
            <option value="lte">Menor ou igual a</option>
          </select>
          <input type="number" value={numValue} onChange={e => setNumValue(e.target.value)} className={`${inputCls} text-[11px]`} />
        </div>
      )}

      <div className="flex justify-end gap-2 mt-2">
        <button onClick={onClose} className="px-2 py-1 rounded-md border border-slate-300 dark:border-slate-700">
          Cancelar
        </button>
        <button onClick={apply} className="px-2 py-1 rounded-md bg-amber-500 text-slate-950 font-bold">
          Filtrar
        </button>
      </div>
    </div>
  );
};
