import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls } from './DialogShell';

const HINT = `Subconjunto de M aceito: [Coluna], literais, + - * / & , comparações, and/or/not,
if … then … else …, null, Text.Upper/Lower/Proper/Trim/Start/End/Middle/Length/Contains,
Number.Round/Abs, Date.Year/Month/Day/Quarter/DayOfWeek/WeekOfMonth/MonthName/DayOfWeekName,
Date.AddDays/AddMonths, Text.From, Number.From, Date.From.`;

export const CustomColumnDialog: React.FC<{
  columns: string[];
  onClose: () => void;
  onApply: (args: { newColumn: string; expression: string }) => void;
}> = ({ columns, onClose, onApply }) => {
  const [newColumn, setNewColumn] = useState('Coluna_Personalizada');
  const [expression, setExpression] = useState('each ');

  const insertColumn = (col: string) => setExpression(expr => `${expr}[${col}]`);

  return (
    <DialogShell
      title="Coluna Personalizada"
      onClose={onClose}
      onConfirm={() => onApply({ newColumn, expression })}
      confirmLabel="OK"
      widthClass="max-w-2xl"
    >
      <div>
        <FieldLabel>Nome da nova coluna</FieldLabel>
        <input value={newColumn} onChange={e => setNewColumn(e.target.value)} className={`${inputCls} w-full`} />
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
