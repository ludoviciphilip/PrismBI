import React, { useState } from 'react';
import { ChevronUp, ChevronDown, X, Settings, AlertTriangle } from 'lucide-react';
import { EtlStep, EtlFailedStep } from '../../types/etl';

export const StepsPanel: React.FC<{
  steps: EtlStep[];
  selectedStepId?: string;
  failedStep?: EtlFailedStep;
  onSelect: (stepId: string) => void;
  onRename: (stepId: string, name: string) => void;
  onDelete: (stepId: string) => void;
  onMoveUp: (stepId: string) => void;
  onMoveDown: (stepId: string) => void;
  onEdit: (stepId: string) => void;
}> = ({ steps, selectedStepId, failedStep, onSelect, onRename, onDelete, onMoveUp, onMoveDown, onEdit }) => {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState('');

  const startRename = (step: EtlStep) => {
    setEditingId(step.id);
    setEditValue(step.name);
  };
  const commitRename = (id: string) => {
    if (editValue.trim()) onRename(id, editValue.trim());
    setEditingId(null);
  };

  return (
    <div className="flex flex-col h-full" data-testid="steps-panel">
      <div className="px-3 py-2 font-bold text-slate-700 dark:text-slate-200 text-[11px] uppercase tracking-wide border-b border-slate-200 dark:border-slate-800">
        Etapas Aplicadas
      </div>
      <div className="flex-1 overflow-y-auto">
        <div className="px-3 py-1.5 text-slate-500 dark:text-slate-400 text-[11px] font-medium">Fonte</div>
        {steps.map((step, idx) => {
          const isSelected = step.id === selectedStepId;
          const isFailed = failedStep?.index === idx;
          return (
            <div
              key={step.id}
              data-testid={`step-row-${step.id}`}
              onClick={() => onSelect(step.id)}
              onDoubleClick={() => startRename(step)}
              className={`group flex items-center gap-1 px-3 py-1.5 cursor-pointer text-[12px] ${
                isSelected ? 'bg-amber-100 dark:bg-amber-900/40 font-semibold text-slate-900 dark:text-slate-50' : 'hover:bg-slate-50 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300'
              }`}
            >
              {isFailed && <AlertTriangle className="w-3 h-3 text-red-500 shrink-0" data-testid={`step-error-${step.id}`} />}
              {editingId === step.id ? (
                <input
                  autoFocus
                  value={editValue}
                  onChange={e => setEditValue(e.target.value)}
                  onBlur={() => commitRename(step.id)}
                  onKeyDown={e => e.key === 'Enter' && commitRename(step.id)}
                  className="flex-1 border border-amber-400 rounded px-1 py-0.5 text-[12px] bg-white dark:bg-slate-900"
                />
              ) : (
                <span className="flex-1 truncate">{step.name}</span>
              )}
              <div className="hidden group-hover:flex items-center gap-0.5 shrink-0">
                <button onClick={e => { e.stopPropagation(); onMoveUp(step.id); }} aria-label="Mover para cima" className="p-0.5 hover:text-amber-600">
                  <ChevronUp className="w-3 h-3" />
                </button>
                <button onClick={e => { e.stopPropagation(); onMoveDown(step.id); }} aria-label="Mover para baixo" className="p-0.5 hover:text-amber-600">
                  <ChevronDown className="w-3 h-3" />
                </button>
                <button onClick={e => { e.stopPropagation(); onEdit(step.id); }} aria-label="Editar etapa" className="p-0.5 hover:text-amber-600">
                  <Settings className="w-3 h-3" />
                </button>
                <button onClick={e => { e.stopPropagation(); onDelete(step.id); }} aria-label="Remover etapa" className="p-0.5 hover:text-red-500">
                  <X className="w-3 h-3" />
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
