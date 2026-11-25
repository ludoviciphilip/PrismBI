import React from 'react';
import { X } from 'lucide-react';

export const DialogShell: React.FC<{
  title: string;
  subtitle?: string;
  onClose: () => void;
  onConfirm?: () => void;
  confirmLabel?: string;
  confirmDisabled?: boolean;
  widthClass?: string;
  children: React.ReactNode;
}> = ({ title, subtitle, onClose, onConfirm, confirmLabel = 'OK', confirmDisabled, widthClass = 'max-w-xl', children }) => (
  <div className="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-4">
    <div className={`bg-white dark:bg-slate-900 rounded-2xl shadow-2xl ${widthClass} w-full text-xs flex flex-col max-h-[90vh] overflow-hidden border border-slate-200 dark:border-slate-800`}>
      <div className="flex items-center justify-between px-5 py-3 border-b border-slate-200 dark:border-slate-800 shrink-0">
        <div>
          <h3 className="font-bold text-slate-900 dark:text-slate-100 text-sm">{title}</h3>
          {subtitle && <p className="text-[11px] text-slate-500 dark:text-slate-400">{subtitle}</p>}
        </div>
        <button
          onClick={onClose}
          aria-label="Fechar"
          className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 font-bold p-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
      <div className="p-5 overflow-y-auto flex-1 space-y-3 min-h-0">{children}</div>
      {onConfirm && (
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-slate-200 dark:border-slate-800 shrink-0">
          <button
            onClick={onClose}
            className="px-3 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 text-slate-600 dark:text-slate-300 font-semibold hover:bg-slate-50 dark:hover:bg-slate-800"
          >
            Cancelar
          </button>
          <button
            onClick={onConfirm}
            disabled={confirmDisabled}
            className="px-3 py-1.5 rounded-lg bg-amber-500 text-slate-950 font-bold hover:bg-amber-400 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {confirmLabel}
          </button>
        </div>
      )}
    </div>
  </div>
);

export const FieldLabel: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <label className="font-semibold text-slate-700 dark:text-slate-200 block mb-1">{children}</label>
);

export const inputCls =
  'w-full border border-slate-300 dark:border-slate-700 rounded-lg p-2 bg-slate-50 dark:bg-slate-800 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500';

export const selectCls = inputCls;
