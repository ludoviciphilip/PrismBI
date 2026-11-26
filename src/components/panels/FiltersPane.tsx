import React, { useMemo, useState } from 'react';
import { Filter as FilterIcon, X, Trash2, Plus, Check, ArrowDownAZ, ArrowDownZA, ArrowDownWideNarrow } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { Filter, Table } from '../../types/bi';
import { useTranslation } from '../../i18n';
import { buildSortByLookup, sortValueList, ValueListSortMode } from '../../engine/visualSort';
import { useUiPrefs } from '../../store/useUiPrefs';
import { CollapsedPaneStrip, PaneCollapseButton } from './PaneCollapseShell';
import { describeSlicerDateFilter } from '../report/slicer';

const MAX_LIST_VALUES = 200;

/** Basic filters (equals / in) get a Power BI-style checklist of the column's values. */
export const isBasicFilter = (f: Filter) => f.operator === 'equals' || f.operator === 'in';

function resolveFilterColumn(filter: Filter, tables: Table[]): { table?: Table; column: string } {
  let tableKey = filter.tableId;
  let column = filter.column;
  if (column.includes('.') && !tables.some(t => (t.id === tableKey || t.name === tableKey) && t.columns.some(c => c.name === column))) {
    const [tPrefix, cName] = column.split('.', 2);
    if (tables.some(t => t.id === tPrefix || t.name === tPrefix)) {
      tableKey = tPrefix;
      column = cName;
    }
  }
  let table = tables.find(t => t.id === tableKey || t.name === tableKey);
  if (table && !table.columns.some(c => c.name.toLowerCase() === column.toLowerCase())) {
    table = tables.find(t => t.columns.some(c => c.name.toLowerCase() === column.toLowerCase())) ?? table;
  }
  const realCol = table?.columns.find(c => c.name.toLowerCase() === column.toLowerCase())?.name ?? column;
  return { table, column: realCol };
}

const SORT_MODES: { mode: ValueListSortMode; icon: React.ElementType }[] = [
  { mode: 'asc', icon: ArrowDownAZ },
  { mode: 'desc', icon: ArrowDownZA },
  { mode: 'count', icon: ArrowDownWideNarrow },
];

export const FilterValueList: React.FC<{
  filter: Filter;
  tables: Table[];
  onChange: (values: string[]) => void;
}> = ({ filter, tables, onChange }) => {
  const { t } = useTranslation();
  const [mode, setMode] = useState<ValueListSortMode>('asc');
  const { table, column } = resolveFilterColumn(filter, tables);

  const items = useMemo(() => {
    if (!table) return [];
    const counts = new Map<string, number>();
    for (const r of table.rows) {
      const v = r[column];
      if (v === null || v === undefined || v === '') continue;
      const k = String(v);
      counts.set(k, (counts.get(k) ?? 0) + 1);
    }
    return Array.from(counts, ([value, count]) => ({ value, count }));
  }, [table, column]);

  const sortKeys = useMemo(() => {
    const meta = table?.columns.find(c => c.name === column);
    return table && meta?.sortByColumn ? buildSortByLookup(table.rows, column, meta.sortByColumn) : undefined;
  }, [table, column]);

  const sorted = useMemo(() => sortValueList(items, mode, sortKeys).slice(0, MAX_LIST_VALUES), [items, mode, sortKeys]);
  const selected = new Set((Array.isArray(filter.value) ? filter.value : [filter.value]).map(v => String(v)));
  const modeLabel = (m: ValueListSortMode) =>
    m === 'asc' ? t.panes.filters.sortAZ : m === 'desc' ? t.panes.filters.sortZA : t.panes.filters.sortByCount;

  if (items.length === 0) return null;

  return (
    <div className="flex flex-col gap-1 mt-0.5">
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-slate-500 dark:text-slate-400">{t.panes.filters.sortValues}</span>
        <div role="group" aria-label={t.panes.filters.sortValues} className="flex items-center rounded border border-slate-300 dark:border-slate-700 overflow-hidden">
          {SORT_MODES.map(({ mode: m, icon: Icon }) => (
            <button
              key={m}
              type="button"
              aria-pressed={mode === m}
              title={modeLabel(m)}
              onClick={() => setMode(m)}
              className={`flex items-center gap-0.5 px-1.5 py-0.5 text-[10px] font-semibold ${
                mode === m
                  ? 'bg-amber-500 text-slate-950'
                  : 'bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-amber-50 dark:hover:bg-slate-700'
              }`}
            >
              <Icon className="w-3 h-3" />
              <span>{m === 'count' ? '#' : modeLabel(m)}</span>
            </button>
          ))}
        </div>
      </div>
      <ul className="max-h-40 overflow-y-auto rounded border border-amber-200/70 dark:border-amber-900/50 bg-white/70 dark:bg-slate-900/50 divide-y divide-slate-100 dark:divide-slate-800" aria-label={filter.column}>
        {sorted.map(it => {
          const key = String(it.value);
          const checked = selected.has(key);
          return (
            <li key={key}>
              <label className="flex items-center gap-1.5 px-1.5 py-0.5 cursor-pointer hover:bg-amber-50 dark:hover:bg-slate-800">
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => {
                    const next = checked ? [...selected].filter(v => v !== key) : [...selected, key];
                    if (next.length > 0) onChange(next);
                  }}
                  className="accent-amber-500 w-3 h-3"
                />
                <span className={`flex-1 truncate ${checked ? 'font-semibold text-amber-900 dark:text-amber-300' : ''}`}>{key}</span>
                <span className="text-[9px] text-slate-400 tabular-nums">{it.count}</span>
              </label>
            </li>
          );
        })}
      </ul>
    </div>
  );
};

export const FiltersPane: React.FC = React.memo(() => {
  const { filters, removeFilter, clearFilters, tables, addFilter, pages } = useBiStore(useShallow(s => ({
    filters: s.filters,
    removeFilter: s.removeFilter,
    clearFilters: s.clearFilters,
    tables: s.tables,
    addFilter: s.addFilter,
    pages: s.pages,
  })));
  const { t } = useTranslation();
  const [showAddFilter, setShowAddFilter] = useState(false);
  const [selectedTableId, setSelectedTableId] = useState(tables[0]?.id || '');
  const [selectedColumn, setSelectedColumn] = useState('');
  const [operator, setOperator] = useState<Filter['operator']>('equals');
  const [filterVal, setFilterVal] = useState('');
  const paneState = useUiPrefs(s => s.panes.filters);
  const togglePaneCollapsed = useUiPrefs(s => s.togglePaneCollapsed);

  if (paneState === 'collapsed') {
    return (
      <CollapsedPaneStrip
        icon={FilterIcon}
        iconClassName="w-4 h-4 text-amber-500"
        title={t.panes.filters.title}
        expandLabel={t.panes.expand}
        onExpand={() => togglePaneCollapsed('filters')}
        testId="filters-pane-collapsed"
      />
    );
  }

  const currentTable = tables.find(t => t.id === selectedTableId || t.name === selectedTableId) || tables[0];
  const columns = currentTable?.columns || [];

  const handleAdd = () => {
    if (!selectedColumn || !filterVal.trim()) return;
    const newFilter: Filter = {
      id: 'filt_' + Math.random().toString(36).substring(2, 7),
      level: 'report',
      tableId: currentTable.id,
      column: selectedColumn,
      operator,
      value: filterVal.trim(),
    };
    addFilter(newFilter);
    setShowAddFilter(false);
    setFilterVal('');
  };

  return (
    <div className="w-64 border-l border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 flex flex-col h-full text-xs select-none text-slate-800 dark:text-slate-200">
      {/* Header */}
      <div className="p-2.5 border-b border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 flex items-center justify-between font-semibold text-slate-700 dark:text-slate-200">
        <div className="flex items-center gap-1.5">
          <FilterIcon className="w-4 h-4 text-amber-500" />
          <span>{t.panes.filters.title}</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setShowAddFilter(!showAddFilter)}
            className="p-1 rounded hover:bg-slate-200 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 flex items-center gap-0.5 text-[11px] font-medium"
            title={t.panes.filters.title}
          >
            <Plus className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />
            <span>+</span>
          </button>
          {filters.length > 0 && (
            <button
              onClick={clearFilters}
              className="p-1 rounded text-red-600 hover:bg-red-50 dark:hover:bg-red-950/40 flex items-center gap-0.5 text-[11px]"
              title={t.panes.filters.clearAll}
            >
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          )}
          <PaneCollapseButton
            collapseLabel={t.panes.collapse}
            onCollapse={() => togglePaneCollapsed('filters')}
            testId="filters-pane-collapse"
          />
        </div>
      </div>

      {/* Add Filter Form */}
      {showAddFilter && (
        <div className="p-3 border-b border-slate-200 dark:border-slate-800 bg-amber-50/50 dark:bg-amber-950/20 space-y-2">
          <div className="flex items-center justify-between font-bold text-amber-900 dark:text-amber-400 text-[11px]">
            <span>Novo Filtro</span>
            <button onClick={() => setShowAddFilter(false)} className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-300">
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
          <div>
            <label className="text-[10px] text-slate-500 dark:text-slate-400 font-semibold block mb-0.5">Tabela</label>
            <select
              value={selectedTableId}
              onChange={(e) => {
                setSelectedTableId(e.target.value);
                setSelectedColumn('');
              }}
              className="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded p-1 text-[11px] text-slate-800 dark:text-slate-100"
            >
              {tables.map(t => (
                <option key={t.id} value={t.id}>{t.name}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="text-[10px] text-slate-500 dark:text-slate-400 font-semibold block mb-0.5">Coluna</label>
            <select
              value={selectedColumn}
              onChange={(e) => setSelectedColumn(e.target.value)}
              className="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded p-1 text-[11px] text-slate-800 dark:text-slate-100"
            >
              <option value="">(Selecione a Coluna)</option>
              {columns.map(c => (
                <option key={c.name} value={c.name}>{c.name}</option>
              ))}
            </select>
          </div>
          <div className="grid grid-cols-2 gap-1.5">
            <div>
              <label className="text-[10px] text-slate-500 dark:text-slate-400 font-semibold block mb-0.5">Operador</label>
              <select
                value={operator}
                onChange={(e) => setOperator(e.target.value as any)}
                className="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded p-1 text-[11px] text-slate-800 dark:text-slate-100"
              >
                <option value="equals">Igual a</option>
                <option value="contains">Contém</option>
                <option value="greaterThan">Maior que</option>
                <option value="lessThan">Menor que</option>
              </select>
            </div>
            <div>
              <label className="text-[10px] text-slate-500 dark:text-slate-400 font-semibold block mb-0.5">Valor</label>
              <input
                type="text"
                placeholder="Ex.: Online"
                value={filterVal}
                onChange={(e) => setFilterVal(e.target.value)}
                className="w-full bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded p-1 text-[11px] text-slate-800 dark:text-slate-100 placeholder-slate-400"
              />
            </div>
          </div>
          <button
            onClick={handleAdd}
            disabled={!selectedColumn || !filterVal.trim()}
            className="w-full mt-1 py-1 rounded bg-amber-500 hover:bg-amber-600 disabled:opacity-50 text-slate-950 font-bold text-[11px] flex items-center justify-center gap-1"
          >
            <Check className="w-3.5 h-3.5" />
            Aplicar Filtro
          </button>
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-2 space-y-2">
        {filters.length === 0 ? (
          <div className="p-4 text-center text-slate-400 dark:text-slate-500 flex flex-col items-center justify-center">
            <FilterIcon className="w-6 h-6 mb-2 text-slate-300 dark:text-slate-600" />
            <p className="font-medium text-slate-500 dark:text-slate-400 text-[11px]">{t.panes.filters.noFilters}</p>
            <p className="text-[10px] text-slate-400 dark:text-slate-500 mt-1">
              Clique em barras ou fatias dos gráficos para filtro cruzado automático, ou adicione um filtro acima.
            </p>
          </div>
        ) : (
          filters.map(filter => (
            <div
              key={filter.id}
              className="p-2 rounded bg-amber-50/70 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-900/60 text-slate-700 dark:text-slate-200 flex flex-col gap-1 text-[11px] relative group"
            >
              <div className="flex items-center justify-between font-semibold text-amber-900 dark:text-amber-400">
                <span className="truncate">{filter.column}</span>
                <button
                  onClick={() => removeFilter(filter.id)}
                  className="p-0.5 text-slate-400 hover:text-red-600 rounded"
                  title="Remover filtro"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
              <div className="text-[10px] text-slate-600 dark:text-slate-400 flex justify-between">
                <span>Tabela: <strong className="text-slate-800 dark:text-slate-200">{filter.tableId}</strong></span>
                <span className="text-[9px] uppercase px-1 rounded bg-slate-200 dark:bg-slate-800 text-slate-600 dark:text-slate-300">
                  {filter.operator === 'equals' ? '=' : filter.operator}
                </span>
              </div>
              <div className="text-[11px] font-bold text-amber-800 dark:text-amber-400 bg-amber-100/70 dark:bg-amber-950/70 px-1.5 py-0.5 rounded truncate">
                {describeSlicerDateFilter(pages, filter.visualId) ??
                  (Array.isArray(filter.value) ? filter.value.join(', ') : String(filter.value))}
              </div>
              {isBasicFilter(filter) && (
                <FilterValueList
                  filter={filter}
                  tables={tables}
                  onChange={values => {
                    const updated: Filter = {
                      ...filter,
                      operator: values.length === 1 ? 'equals' : 'in',
                      value: values.length === 1 ? values[0] : values,
                    };
                    // Replace in place so the card keeps its position in the pane.
                    useBiStore.setState(st => ({ filters: st.filters.map(f => (f.id === filter.id ? updated : f)) }));
                  }}
                />
              )}
            </div>
          ))
        )}

        {/* Section: Status */}
        <div className="pt-2 border-t border-slate-100 dark:border-slate-800">
          <span className="text-[10px] font-bold uppercase tracking-wider text-slate-400 dark:text-slate-500 block mb-1">
            Propagação Automática
          </span>
          <div className="p-2 rounded bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-700/80 text-[11px] text-slate-500 dark:text-slate-400">
            Filtros cruzados bidirecionais ativos e sincronizados entre todas as páginas e gráficos.
          </div>
        </div>
      </div>
    </div>
  );
});
