import React, { useMemo, useState } from 'react';
import { Column, Table, Visual } from '../../../types/bi';
import { useBiStore } from '../../../store/useBiStore';
import { useTranslation } from '../../../i18n';
import { BetweenSlicer } from './BetweenSlicer';
import { RelativeDateSlicer } from './RelativeDateSlicer';
import { filterItemsBySearch } from './slicerSearch';

const DEFAULT_RELATIVE = { anchor: 'last' as const, count: 7, unit: 'days' as const, includeToday: true };

/** Search box shared by list/dropdown/tiles modes (Power BI "showSearch"). */
const SearchBox: React.FC<{ query: string; onChange: (q: string) => void }> = ({ query, onChange }) => {
  const { t } = useTranslation();
  return (
    <input
      type="text"
      value={query}
      onChange={e => onChange(e.target.value)}
      placeholder={t.slicer.searchPlaceholder}
      aria-label={t.slicer.searchPlaceholder}
      className="w-full text-[11px] bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-slate-800 dark:text-slate-100 placeholder-slate-400"
    />
  );
};

/**
 * Renders a slicer visual's body for every `slicerConfig.mode`. Owns the
 * search box + multi-select state for list/dropdown/tiles, and delegates
 * 'between'/'relative-date' to their dedicated components. All selection
 * changes go through `setSlicerSelection`, which also handles syncGroup
 * propagation to other pages.
 */
export const SlicerBody: React.FC<{
  visual: Visual;
  table?: Table;
  column?: Column;
  categories: string[];
}> = ({ visual, table, column, categories }) => {
  const { t } = useTranslation();
  const setSlicerSelection = useBiStore(s => s.setSlicerSelection);
  const [query, setQuery] = useState('');
  const slicerConfig = visual.dataConfig.slicerConfig;
  const mode = slicerConfig?.mode || 'dropdown';
  const showSearch = Boolean(slicerConfig?.showSearch) && (mode === 'list' || mode === 'dropdown' || mode === 'tiles');
  const selected = useMemo(() => new Set((slicerConfig?.selectedValues || []).map(v => String(v))), [slicerConfig?.selectedValues]);
  const filtered = useMemo(() => filterItemsBySearch(categories, query), [categories, query]);

  const toggleValue = (val: string) => {
    const next = selected.has(val) ? [...selected].filter(v => v !== val) : [...selected, val];
    setSlicerSelection(visual.id, { selectedValues: next });
  };

  const allFilteredSelected = filtered.length > 0 && filtered.every(c => selected.has(c));
  const toggleSelectAll = () => {
    if (allFilteredSelected) {
      setSlicerSelection(visual.id, { selectedValues: [] });
    } else {
      const next = new Set(selected);
      filtered.forEach(c => next.add(c));
      setSlicerSelection(visual.id, { selectedValues: [...next] });
    }
  };

  if (mode === 'between' && table && column) {
    return (
      <BetweenSlicer
        table={table}
        column={column}
        value={slicerConfig?.between || {}}
        onChange={between => setSlicerSelection(visual.id, { between })}
      />
    );
  }

  if (mode === 'relative-date') {
    return (
      <RelativeDateSlicer
        value={slicerConfig?.relative || DEFAULT_RELATIVE}
        onChange={relative => setSlicerSelection(visual.id, { relative })}
      />
    );
  }

  if (mode === 'list') {
    return (
      <div className="flex flex-col gap-1 min-w-0">
        {showSearch && <SearchBox query={query} onChange={setQuery} />}
        <label className="flex items-center gap-1.5 px-1 text-[11px] font-semibold text-slate-600 dark:text-slate-300 cursor-pointer">
          <input type="checkbox" checked={allFilteredSelected} onChange={toggleSelectAll} className="accent-amber-500 w-3 h-3" />
          {t.slicer.selectAll}
        </label>
        <ul className="max-h-32 overflow-y-auto rounded border border-slate-200 dark:border-slate-700 divide-y divide-slate-100 dark:divide-slate-800">
          {filtered.map(c => (
            <li key={c}>
              <label className="flex items-center gap-1.5 px-1.5 py-0.5 text-[11px] cursor-pointer hover:bg-amber-50 dark:hover:bg-slate-800">
                <input type="checkbox" checked={selected.has(c)} onChange={() => toggleValue(c)} className="accent-amber-500 w-3 h-3" />
                <span className="truncate">{c}</span>
              </label>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  if (mode === 'tiles') {
    return (
      <div className="flex flex-col gap-1.5 min-w-0">
        {showSearch && <SearchBox query={query} onChange={setQuery} />}
        <div className="flex flex-wrap gap-1.5">
          <button
            type="button"
            onClick={toggleSelectAll}
            className={`px-2 py-1 rounded text-[11px] font-semibold border transition-colors cursor-pointer ${
              allFilteredSelected
                ? 'bg-amber-500 text-slate-950 border-amber-500'
                : 'bg-slate-50 dark:bg-slate-800 text-slate-700 dark:text-slate-300 border-slate-300 dark:border-slate-700 hover:border-amber-400'
            }`}
          >
            {t.slicer.selectAll}
          </button>
          {filtered.map(c => {
            const isSel = selected.has(c);
            return (
              <button
                key={c}
                type="button"
                onClick={() => toggleValue(c)}
                className={`px-2 py-1 rounded text-[11px] font-medium border transition-colors cursor-pointer ${
                  isSel
                    ? 'bg-amber-500 text-slate-950 border-amber-500 font-bold'
                    : 'bg-slate-50 dark:bg-slate-800 text-slate-700 dark:text-slate-300 border-slate-300 dark:border-slate-700 hover:border-amber-400'
                }`}
              >
                {c}
              </button>
            );
          })}
        </div>
      </div>
    );
  }

  // 'dropdown' (and 'slider', which has no dedicated UI yet): classic single-value select.
  const activeVal = slicerConfig?.selectedValues?.[0] !== undefined ? String(slicerConfig.selectedValues[0]) : 'TODOS';
  return (
    <div className="flex flex-col gap-1">
      {showSearch && <SearchBox query={query} onChange={setQuery} />}
      <select
        value={activeVal}
        aria-label={column?.name}
        className="w-full text-xs bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500 font-medium"
        onChange={e => setSlicerSelection(visual.id, { selectedValues: e.target.value === 'TODOS' ? [] : [e.target.value] })}
      >
        <option value="TODOS">{t.visualCard.allRecords}</option>
        {filtered.map(c => (
          <option key={c} value={c}>
            {c}
          </option>
        ))}
      </select>
    </div>
  );
};
