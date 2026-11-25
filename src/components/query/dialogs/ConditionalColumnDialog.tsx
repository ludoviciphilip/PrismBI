import React, { useState } from 'react';
import { Plus, Trash2 } from 'lucide-react';
import { DialogShell, FieldLabel, inputCls, selectCls } from './DialogShell';
import { Cond } from '../../../types/etl';

interface Rule {
  column: string;
  op: string;
  value: string;
  thenValue: string;
}

export const ConditionalColumnDialog: React.FC<{
  columns: string[];
  onClose: () => void;
  onApply: (args: { newColumn: string; rules: { when: Cond; then: string }[]; otherwise: string }) => void;
}> = ({ columns, onClose, onApply }) => {
  const [newColumn, setNewColumn] = useState('Coluna_Condicional');
  const [rules, setRules] = useState<Rule[]>([{ column: columns[0] || '', op: 'eq', value: '', thenValue: '' }]);
  const [otherwise, setOtherwise] = useState('');

  const addRule = () => setRules([...rules, { column: columns[0] || '', op: 'eq', value: '', thenValue: '' }]);
  const removeRule = (i: number) => setRules(rules.filter((_, idx) => idx !== i));
  const update = (i: number, patch: Partial<Rule>) => setRules(rules.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));

  const handleApply = () => {
    onApply({
      newColumn,
      rules: rules.map(r => ({ when: { column: r.column, op: r.op as any, value: r.value }, then: r.thenValue })),
      otherwise,
    });
  };

  return (
    <DialogShell title="Coluna Condicional" onClose={onClose} onConfirm={handleApply} confirmLabel="OK" widthClass="max-w-2xl">
      <div>
        <FieldLabel>Nome da nova coluna</FieldLabel>
        <input value={newColumn} onChange={e => setNewColumn(e.target.value)} className={`${inputCls} w-full`} />
      </div>
      <div className="space-y-2">
        {rules.map((r, i) => (
          <div key={i} className="flex items-center gap-2">
            <span className="text-slate-400">Se</span>
            <select value={r.column} onChange={e => update(i, { column: e.target.value })} className={`${selectCls} flex-1`}>
              {columns.map(c => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
            <select value={r.op} onChange={e => update(i, { op: e.target.value })} className={`${selectCls} w-32`}>
              <option value="eq">é igual a</option>
              <option value="neq">é diferente de</option>
              <option value="gt">maior que</option>
              <option value="lt">menor que</option>
              <option value="contains">contém</option>
            </select>
            <input value={r.value} onChange={e => update(i, { value: e.target.value })} className={`${inputCls} flex-1`} placeholder="valor" />
            <span className="text-slate-400">então</span>
            <input value={r.thenValue} onChange={e => update(i, { thenValue: e.target.value })} className={`${inputCls} flex-1`} placeholder="saída" />
            <button onClick={() => removeRule(i)} className="text-slate-400 hover:text-red-500">
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          </div>
        ))}
        <button onClick={addRule} className="text-[11px] font-semibold text-amber-600 flex items-center gap-1">
          <Plus className="w-3 h-3" /> Adicionar cláusula
        </button>
      </div>
      <div>
        <FieldLabel>Caso contrário</FieldLabel>
        <input value={otherwise} onChange={e => setOtherwise(e.target.value)} className={`${inputCls} w-full`} />
      </div>
    </DialogShell>
  );
};
