import React, { useState } from 'react';
import { DialogShell } from './DialogShell';

export const AdvancedEditorModal: React.FC<{
  m: string;
  warnings?: string[];
  onClose: () => void;
  onApply: (m: string) => void;
}> = ({ m, warnings, onClose, onApply }) => {
  const [text, setText] = useState(m);
  return (
    <DialogShell title="Editor Avançado" subtitle="Script M completo da consulta" onClose={onClose} onConfirm={() => onApply(text)} confirmLabel="Concluído" widthClass="max-w-3xl">
      {warnings && warnings.length > 0 && (
        <div className="bg-amber-50 dark:bg-amber-900/30 border border-amber-300 dark:border-amber-700 rounded-lg p-2 text-[11px] text-amber-800 dark:text-amber-200 space-y-0.5">
          {warnings.map((w, i) => (
            <div key={i}>⚠ {w}</div>
          ))}
        </div>
      )}
      <textarea
        data-testid="advanced-editor-textarea"
        value={text}
        onChange={e => setText(e.target.value)}
        className="w-full h-96 font-mono text-[11px] border border-slate-300 dark:border-slate-700 rounded-lg p-3 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500"
        spellCheck={false}
      />
    </DialogShell>
  );
};
