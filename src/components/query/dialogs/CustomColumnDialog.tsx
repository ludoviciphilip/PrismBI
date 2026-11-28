import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls, selectCls } from './DialogShell';
import { EtlType } from '../../../types/etl';

const HINT = `Subconjunto de M aceito: [Coluna], literais, + - * / & , comparações, and/or/not,
if … then … else …, null, Text.Upper/Lower/Proper/Trim/Start/End/Middle/Length/Contains,
Number.Round/Abs, Date.Year/Month/Day/Quarter/DayOfWeek/WeekOfMonth/MonthName/DayOfWeekName,
Date.AddDays/AddMonths, Text.From, Number.From, Date.From.`;

const TYPES: { value: EtlType | ''; label: string }[] = [
  { value: '', label: 'Automático' },
  { value: 'text', label: 'Texto' },
  { value: 'integer', label: 'Número Inteiro' },
  { value: 'decimal', label: 'Número Decimal' },
  { value: 'currency', label: 'Moeda' },
  { value: 'date', label: 'Data' },
  { value: 'datetime', label: 'Data/Hora' },
  { value: 'boolean', label: 'Booleano (Verdadeiro/Falso)' },
];

/** Tira um "each " de abertura: a etapa guarda só a fórmula, sem o `each` do M. */
function stripEach(expression: string): string {
  return expression.replace(/^\s*each\s+/, '');
}

export const CustomColumnDialog: React.FC<{
  columns: string[];
  /** Dados da etapa quando reaberta pela engrenagem (editar), nunca com 'each'. */
  initial?: { newColumn: string; expression: string; type?: EtlType };
  onClose: () => void;
  onApply: (args: { newColumn: string; expression: string; type?: EtlType }) => void;
}> = ({ columns, initial, onClose, onApply }) => {
  const [newColumn, setNewColumn] = useState(initial?.newColumn ?? 'Coluna_Personalizada');
  const [expression, setExpression] = useState(initial ? stripEach(initial.expression) : '');
  const [type, setType] = useState<EtlType | ''>(initial?.type ?? '');

  const insertColumn = (col: string) => setExpression(expr => `${expr}[${col}]`);

  return (
    <DialogShell
      title="Coluna Personalizada"
      onClose={onClose}
      onConfirm={() => onApply({ newColumn, expression, ...(type ? { type } : {}) })}
      confirmLabel="OK"
      widthClass="max-w-2xl"
    >
      <div className="grid grid-cols-2 gap-3">
        <div>
          <FieldLabel>Nome da nova coluna</FieldLabel>
          <input value={newColumn} onChange={e => setNewColumn(e.target.value)} className={`${inputCls} w-full`} />
        </div>
        <div>
          <FieldLabel>Tipo de dados de saída</FieldLabel>
          <select value={type} onChange={e => setType(e.target.value as EtlType | '')} className={`${selectCls} w-full`}>
            {TYPES.map(t => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </select>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div className="col-span-2">
          <FieldLabel>Fórmula da coluna personalizada</FieldLabel>
          <textarea
            data-testid="custom-column-expression"
            value={expression}
            onChange={e => setExpression(e.target.value)}
            className={`${inputCls} font-mono h-32 w-full`}
          />
          <p className="text-[10px] text-slate-400 mt-1 whitespace-pre-line">{HINT}</p>
        </div>
        <div>
          <FieldLabel>Colunas disponíveis</FieldLabel>
          <div className="border border-slate-200 dark:border-slate-700 rounded-lg max-h-32 overflow-y-auto">
            {columns.map(c => (
              <div
                key={c}
                onClick={() => insertColumn(c)}
                className="px-2 py-1 cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-800"
              >
                {c}
              </div>
            ))}
          </div>
        </div>
      </div>
    </DialogShell>
  );
};
