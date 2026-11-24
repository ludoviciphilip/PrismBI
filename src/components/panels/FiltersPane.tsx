import React, { useState } from 'react';
import { Filter as FilterIcon, X, Trash2, Plus, Check } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useBiStore } from '../../store/useBiStore';
import { Filter } from '../../types/bi';
import { useTranslation } from '../../i18n';

export const FiltersPane: React.FC = React.memo(() => {
  const { filters, removeFilter, clearFilters, tables, addFilter } = useBiStore(useShallow(s => ({
    filters: s.filters,
    removeFilter: s.removeFilter,
    clearFilters: s.clearFilters,
    tables: s.tables,
    addFilter: s.addFilter,
  })));
  const { t } = useTranslation();
  const [showAddFilter, setShowAddFilter] = useState(false);
  const [selectedTableId, setSelectedTableId] = useState(tables[0]?.id || '');
  const [selectedColumn, setSelectedColumn] = useState('');
  const [operator, setOperator] = useState<Filter['operator']>('equals');
  const [filterVal, setFilterVal] = useState('');

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
                {String(filter.value)}
              </div>
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
