/**
 * Lista "Formatação condicional" do painel de formato (VisualizationsPane):
 * mostra os `ConditionalFormat` do visual selecionado, com adicionar,
 * editar e remover, abrindo `ConditionalFormatDialog`.
 */
import React, { useState } from 'react';
import { Pencil, Plus, Trash2, Palette } from 'lucide-react';
import type { ConditionalFormat, Visual } from '../../../types/bi';
import { ConditionalFormatDialog } from './ConditionalFormatDialog';

const TARGET_LABELS: Record<ConditionalFormat['target'], string> = {
  background: 'Cor de fundo',
  fontColor: 'Cor da fonte',
  dataBar: 'Barra de dados',
  icon: 'Ícones',
};

export interface ConditionalFormatListProps {
  visual: Visual;
  availableFields: string[];
  measureFields: string[];
  onChange: (formats: ConditionalFormat[]) => void;
}

export const ConditionalFormatList: React.FC<ConditionalFormatListProps> = ({
  visual,
  availableFields,
  measureFields,
  onChange,
}) => {
  const formats = visual.formatConfig.conditionalFormats || [];
  const [dialogState, setDialogState] = useState<{ open: boolean; editing: ConditionalFormat | null }>({
    open: false,
    editing: null,
  });

  if (availableFields.length === 0) {
    return (
      <p className="text-[11px] text-slate-500 dark:text-slate-400 italic">
        Selecione ao menos um campo neste visual para configurar a formatação condicional.
      </p>
    );
  }

  const handleSave = (format: ConditionalFormat) => {
    const exists = formats.some(f => f.id === format.id);
    const next = exists ? formats.map(f => (f.id === format.id ? format : f)) : [...formats, format];
    onChange(next);
    setDialogState({ open: false, editing: null });
  };

  const handleRemove = (id: string) => {
    onChange(formats.filter(f => f.id !== id));
  };

  return (
    <div className="space-y-2">
      {formats.length === 0 && (
        <p className="text-[11px] text-slate-500 dark:text-slate-400 italic">Nenhuma formatação condicional configurada.</p>
      )}
      {formats.map(f => (
        <div
          key={f.id}
          className="flex items-center justify-between gap-2 px-2 py-1.5 rounded border border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-800/40"
        >
          <div className="flex items-center gap-1.5 min-w-0">
            <Palette className="w-3 h-3 text-amber-500 flex-shrink-0" />
            <span className="text-[11px] font-semibold text-slate-700 dark:text-slate-200 truncate" title={f.applyTo}>
              {f.applyTo}
            </span>
            <span className="text-[10px] text-slate-400">·</span>
            <span className="text-[11px] text-slate-500 dark:text-slate-400 truncate">{TARGET_LABELS[f.target]}</span>
          </div>
          <div className="flex items-center gap-1 flex-shrink-0">
            <button
              type="button"
              title="Editar"
              aria-label={`Editar formatação de ${f.applyTo}`}
              onClick={() => setDialogState({ open: true, editing: f })}
              className="p-1 text-slate-400 hover:text-amber-600 rounded hover:bg-slate-200 dark:hover:bg-slate-700 cursor-pointer"
            >
              <Pencil className="w-3 h-3" />
            </button>
            <button
              type="button"
              title="Remover"
              aria-label={`Remover formatação de ${f.applyTo}`}
              onClick={() => handleRemove(f.id)}
              className="p-1 text-slate-400 hover:text-rose-600 rounded hover:bg-slate-200 dark:hover:bg-slate-700 cursor-pointer"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
        </div>
      ))}
      <button
        type="button"
        onClick={() => setDialogState({ open: true, editing: null })}
        className="flex items-center gap-1 text-[11px] text-amber-600 hover:text-amber-700 font-semibold cursor-pointer"
      >
        <Plus className="w-3.5 h-3.5" /> Adicionar formatação condicional
      </button>

      {dialogState.open && (
        <ConditionalFormatDialog
          visual={visual}
          availableFields={availableFields}
          measureFields={measureFields}
          initial={dialogState.editing}
          onSave={handleSave}
          onClose={() => setDialogState({ open: false, editing: null })}
        />
      )}
    </div>
  );
};
