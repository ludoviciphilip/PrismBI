import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls } from './DialogShell';

export const IndexColumnDialog: React.FC<{
  onClose: () => void;
  onApply: (args: { newColumn: string; start: number; step: number }) => void;
}> = ({ onClose, onApply }) => {
  const [newColumn, setNewColumn] = useState('Índice');
  const [start, setStart] = useState(0);
  const [step, setStep] = useState(1);

  return (
    <DialogShell title="Coluna de Índice" onClose={onClose} onConfirm={() => onApply({ newColumn, start, step })} confirmLabel="OK">
      <div>
        <FieldLabel>Nome da coluna</FieldLabel>
        <input value={newColumn} onChange={e => setNewColumn(e.target.value)} className={`${inputCls} w-full`} />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div>
          <FieldLabel>Valor inicial</FieldLabel>
          <input type="number" value={start} onChange={e => setStart(Number(e.target.value))} className={`${inputCls} w-full`} />
        </div>
        <div>
          <FieldLabel>Incremento</FieldLabel>
          <input type="number" value={step} onChange={e => setStep(Number(e.target.value))} className={`${inputCls} w-full`} />
        </div>
      </div>
    </DialogShell>
  );
};
