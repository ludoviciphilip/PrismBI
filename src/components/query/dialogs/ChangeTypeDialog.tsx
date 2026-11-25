import React, { useState } from 'react';
import { DialogShell, selectCls } from './DialogShell';
import { EtlType } from '../../../types/etl';

const TYPES: { value: EtlType; label: string }[] = [
  { value: 'text', label: 'Texto' },
  { value: 'integer', label: 'Número Inteiro' },
  { value: 'decimal', label: 'Número Decimal' },
  { value: 'currency', label: 'Moeda' },
  { value: 'date', label: 'Data' },
  { value: 'datetime', label: 'Data/Hora' },
  { value: 'boolean', label: 'Booleano (Verdadeiro/Falso)' },
];

export const ChangeTypeDialog: React.FC<{
  column: string;
  currentType?: EtlType;
  onClose: () => void;
  onApply: (type: EtlType) => void;
}> = ({ column, currentType, onClose, onApply }) => {
  const [type, setType] = useState<EtlType>(currentType || 'text');
  return (
    <DialogShell title="Alterar Tipo" subtitle={`Coluna: ${column}`} onClose={onClose} onConfirm={() => onApply(type)} confirmLabel="OK">
      <select data-testid="change-type-select" value={type} onChange={e => setType(e.target.value as EtlType)} className={`${selectCls} w-full`}>
        {TYPES.map(t => (
          <option key={t.value} value={t.value}>
            {t.label}
          </option>
        ))}
      </select>
    </DialogShell>
  );
};
