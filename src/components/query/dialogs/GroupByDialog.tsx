import React, { useState } from 'react';
import { Plus, Trash2 } from 'lucide-react';
import { DialogShell, inputCls, selectCls } from './DialogShell';
import { Agg, AggOp } from '../../../types/etl';

const AGG_OPS: { value: AggOp; label: string }[] = [
  { value: 'sum', label: 'Soma' },
  { value: 'avg', label: 'Média' },
  { value: 'min', label: 'Mínimo' },
  { value: 'max', label: 'Máximo' },
  { value: 'count', label: 'Contagem' },
  { value: 'countDistinct', label: 'Contagem distinta' },
  { value: 'countRows', label: 'Contagem de linhas' },
  { value: 'median', label: 'Mediana' },
];

export const GroupByDialog: React.FC<{
  columns: string[];
  onClose: () => void;
  onApply: (args: { keys: string[]; aggregations: Agg[] }) => void;
}> = ({ columns, onClose, onApply }) => {
  const [keys, setKeys] = useState<string[]>(columns.slice(0, 1));
  const [aggs, setAggs] = useState<Agg[]>([{ name: `${columns[0] || 'Contagem'} - Soma`, op: 'sum', column: columns[0] }]);

  const toggleKey = (col: string) => {
    setKeys(ks => (ks.includes(col) ? ks.filter(k => k !== col) : [...ks, col]));
  };

  const addAgg = () => setAggs([...aggs, { name: 'Nova Coluna', op: 'countRows' }]);
  const removeAgg = (i: number) => setAggs(aggs.filter((_, idx) => idx !== i));
  const updateAgg = (i: number, patch: Partial<Agg>) => setAggs(aggs.map((a, idx) => (idx === i ? { ...a, ...patch } : a)));

  return (
    <DialogShell
      title="Agrupar Por"
      onClose={onClose}
      onConfirm={() => onApply({ keys, aggregations: aggs })}
      confirmLabel="OK"
      widthClass="max-w-2xl"
    >
      <div>
        <div className="font-semibold text-slate-600 dark:text-slate-300 mb-1">Colunas de agrupamento</div>
        <div className="flex flex-wrap gap-2" data-testid="groupby-keys">
          {columns.map(c => (
            <label key={c} className="flex items-center gap-1 border border-slate-300 dark:border-slate-700 rounded-lg px-2 py-1">
              <input type="checkbox" checked={keys.includes(c)} onChange={() => toggleKey(c)} />
              {c}
            </label>
          ))}
        </div>
      </div>

      <div>
        <div className="flex items-center justify-between mb-1">
          <span className="font-semibold text-slate-600 dark:text-slate-300">Agregações</span>
          <button onClick={addAgg} className="text-amber-600 hover:underline text-[11px] flex items-center gap-1">
            <Plus className="w-3 h-3" /> Nova agregação
          </button>
        </div>
        <div className="space-y-2">
          {aggs.map((agg, i) => (
            <div key={i} className="flex items-center gap-2" data-testid={`agg-row-${i}`}>
              <input
                value={agg.name}
                onChange={e => updateAgg(i, { name: e.target.value })}
                className={`${inputCls} flex-1`}
                placeholder="Nome da nova coluna"
              />
              <select value={agg.op} onChange={e => updateAgg(i, { op: e.target.value as AggOp })} className={`${selectCls} w-40`}>
                {AGG_OPS.map(o => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
              {agg.op !== 'countRows' && (
                <select value={agg.column || ''} onChange={e => updateAgg(i, { column: e.target.value })} className={`${selectCls} flex-1`}>
                  {columns.map(c => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
              )}
              <button onClick={() => removeAgg(i)} className="text-slate-400 hover:text-red-500">
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </div>
          ))}
        </div>
      </div>
    </DialogShell>
  );
};
