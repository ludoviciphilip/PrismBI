import React, { useState } from 'react';
import { DialogShell, FieldLabel, selectCls } from './DialogShell';
import { AggOp } from '../../../types/etl';

export const PivotDialog: React.FC<{
  columns: string[];
  onClose: () => void;
  onApply: (args: { column: string; valuesColumn: string; aggregation: AggOp }) => void;
}> = ({ columns, onClose, onApply }) => {
  const [column, setColumn] = useState(columns[0] || '');
  const [valuesColumn, setValuesColumn] = useState(columns[1] || columns[0] || '');
  const [aggregation, setAggregation] = useState<AggOp>('sum');

  return (
    <DialogShell title="Dinamizar Coluna (Pivot)" onClose={onClose} onConfirm={() => onApply({ column, valuesColumn, aggregation })} confirmLabel="OK">
      <div>
        <FieldLabel>Coluna dinâmica (vira cabeçalhos)</FieldLabel>
        <select value={column} onChange={e => setColumn(e.target.value)} className={`${selectCls} w-full`}>
          {columns.map(c => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </div>
      <div>
        <FieldLabel>Coluna de valores</FieldLabel>
        <select value={valuesColumn} onChange={e => setValuesColumn(e.target.value)} className={`${selectCls} w-full`}>
          {columns.map(c => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </div>
      <div>
        <FieldLabel>Função de agregação de valores</FieldLabel>
        <select value={aggregation} onChange={e => setAggregation(e.target.value as AggOp)} className={`${selectCls} w-full`}>
          <option value="sum">Soma</option>
          <option value="avg">Média</option>
          <option value="min">Mínimo</option>
          <option value="max">Máximo</option>
          <option value="count">Contagem</option>
        </select>
      </div>
    </DialogShell>
  );
};
