import React, { useState } from 'react';
import { DialogShell, FieldLabel, inputCls } from './DialogShell';
import { ParamFieldSpec } from '../ribbon/opDefs';

/**
 * Diálogo compacto reutilizável para os prompts de parâmetro do ribbon
 * (número de caracteres, delimitador, fator, prefixo, intervalo…), dirigido
 * por uma lista de ParamFieldSpec.
 */
export const ParamDialog: React.FC<{
  title: string;
  subtitle?: string;
  fields: ParamFieldSpec[];
  onClose: () => void;
  onApply: (values: Record<string, any>) => void;
}> = ({ title, subtitle, fields, onClose, onApply }) => {
  const [values, setValues] = useState<Record<string, any>>(() => {
    const init: Record<string, any> = {};
    fields.forEach(f => {
      init[f.key] = f.defaultValue ?? (f.type === 'text' ? '' : 0);
    });
    return init;
  });

  const setValue = (key: string, raw: string, type: ParamFieldSpec['type']) => {
    setValues(v => ({ ...v, [key]: type === 'text' ? raw : Number(raw) }));
  };

  return (
    <DialogShell title={title} subtitle={subtitle} onClose={onClose} onConfirm={() => onApply(values)} confirmLabel="OK" widthClass="max-w-sm">
      {fields.map(f => (
        <div key={f.key}>
          <FieldLabel>{f.label}</FieldLabel>
          <input
            data-testid={`param-field-${f.key}`}
            type={f.type === 'text' ? 'text' : 'number'}
            step={f.type === 'integer' ? 1 : 'any'}
            value={values[f.key]}
            onChange={e => setValue(f.key, e.target.value, f.type)}
            className={`${inputCls} w-full`}
          />
        </div>
      ))}
    </DialogShell>
  );
};
