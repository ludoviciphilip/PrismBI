import React, { useEffect, useRef, useState } from 'react';
import { MoreHorizontal, Check, ChevronRight, ArrowDownWideNarrow, ArrowUpNarrowWide } from 'lucide-react';
import { useTranslation } from '../../i18n';

export interface SortableField {
  /** Value stored in dataConfig.sortField. */
  field: string;
  /** Label shown in the menu. */
  label: string;
}

interface VisualSortMenuProps {
  /** Fields offered under "Classificar por ▸". Empty → only the direction items are shown (slicer). */
  fields: SortableField[];
  /** Field currently shown as checked (the explicit or effective sort field). */
  activeField?: string;
  activeOrder: 'asc' | 'desc';
  onSelectField: (field: string) => void;
  onSelectOrder: (order: 'asc' | 'desc') => void;
}

/**
 * Power BI-style "Mais opções (...)" header menu with "Classificar por ▸ <campo>"
 * and "Ordem crescente / decrescente", with check marks on the current choice.
 */
export const VisualSortMenu: React.FC<VisualSortMenuProps> = ({
  fields,
  activeField,
  activeOrder,
  onSelectField,
  onSelectOrder,
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [subOpen, setSubOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
        setSubOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(false);
        setSubOpen(false);
      }
    };
    document.addEventListener('mousedown', onDocDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDocDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const itemCls =
    'w-full flex items-center gap-2 px-2.5 py-1.5 text-left text-[11px] text-slate-700 dark:text-slate-200 hover:bg-amber-50 dark:hover:bg-slate-700 cursor-pointer';
  const checkSlot = (on: boolean) => (
    <span className="w-3.5 h-3.5 shrink-0 flex items-center justify-center">
      {on && <Check className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" aria-hidden />}
    </span>
  );

  return (
    <div ref={rootRef} className="relative" onMouseDown={e => e.stopPropagation()} onClick={e => e.stopPropagation()}>
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={t.visualCard.moreOptions}
        title={t.visualCard.moreOptions}
        onClick={() => {
          setOpen(o => !o);
          setSubOpen(false);
        }}
        className={`p-1 rounded ${
          open
            ? 'text-amber-600 bg-amber-100 dark:bg-amber-950/60'
            : 'text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-100 hover:bg-slate-200/60 dark:hover:bg-slate-700/60'
        }`}
      >
        <MoreHorizontal className="w-3.5 h-3.5" />
      </button>

      {open && (
        <div
          role="menu"
          aria-label={t.visualCard.moreOptions}
          className="absolute right-0 top-full mt-1 z-[60] min-w-[210px] py-1 rounded-md border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-xl"
        >
          {fields.length > 0 && (
            <div className="relative" onMouseEnter={() => setSubOpen(true)} onMouseLeave={() => setSubOpen(false)}>
              <button
                type="button"
                role="menuitem"
                aria-haspopup="menu"
                aria-expanded={subOpen}
                onClick={() => setSubOpen(s => !s)}
                className={`${itemCls} justify-between`}
              >
                <span className="flex items-center gap-2">
                  <span className="w-3.5 h-3.5 shrink-0" />
                  {t.visualCard.sortBy}
                </span>
                <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
              </button>
              {subOpen && (
                <div
                  role="menu"
                  aria-label={t.visualCard.sortBy}
                  className="absolute right-full top-0 mr-0.5 min-w-[180px] max-h-64 overflow-y-auto py-1 rounded-md border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-xl"
                >
                  {fields.map(f => {
                    const checked = activeField === f.field;
                    return (
                      <button
                        key={f.field}
                        type="button"
                        role="menuitemradio"
                        aria-checked={checked}
                        onClick={() => {
                          onSelectField(f.field);
                          setOpen(false);
                          setSubOpen(false);
                        }}
                        className={`${itemCls} ${checked ? 'font-semibold' : ''}`}
                      >
                        {checkSlot(checked)}
                        <span className="truncate">{f.label}</span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          )}
          {fields.length > 0 && <div className="my-1 border-t border-slate-100 dark:border-slate-700" />}
          {(['desc', 'asc'] as const).map(order => {
            const checked = activeOrder === order;
            return (
              <button
                key={order}
                type="button"
                role="menuitemradio"
                aria-checked={checked}
                onClick={() => {
                  onSelectOrder(order);
                  setOpen(false);
                }}
                className={`${itemCls} ${checked ? 'font-semibold' : ''}`}
              >
                {checkSlot(checked)}
                {order === 'desc' ? (
                  <ArrowDownWideNarrow className="w-3.5 h-3.5 text-slate-400" />
                ) : (
                  <ArrowUpNarrowWide className="w-3.5 h-3.5 text-slate-400" />
                )}
                <span>{order === 'desc' ? t.visualCard.sortDescending : t.visualCard.sortAscending}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
};
