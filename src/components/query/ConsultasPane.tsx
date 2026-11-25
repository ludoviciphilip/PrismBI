import React, { useState } from 'react';
import { ChevronsLeft, ChevronsRight, Search, Table2 } from 'lucide-react';

export interface QueryListEntry {
  id: string;
  name: string;
  stepCount: number;
}

/**
 * Painel esquerdo "Consultas [N]": lista as consultas com ícone de tabela,
 * selo de nº de etapas e destaque da selecionada, recolhível («) e com
 * busca — substitui o <select> no canto da ribbon.
 */
export const ConsultasPane: React.FC<{
  queries: QueryListEntry[];
  selectedId: string;
  onSelect: (id: string) => void;
}> = ({ queries, selectedId, onSelect }) => {
  const [collapsed, setCollapsed] = useState(false);
  const [search, setSearch] = useState('');

  const filtered = queries.filter(q => q.name.toLowerCase().includes(search.toLowerCase()));

  if (collapsed) {
    return (
      <div className="w-7 border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shrink-0 flex flex-col items-center py-2">
        <button
          data-testid="consultas-expand"
          onClick={() => setCollapsed(false)}
          aria-label="Expandir painel de consultas"
          className="p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500"
        >
          <ChevronsRight className="w-3.5 h-3.5" />
        </button>
      </div>
    );
  }

  return (
    <div className="w-56 border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shrink-0 flex flex-col" data-testid="consultas-pane">
      <div className="flex items-center justify-between px-2 py-1.5 border-b border-slate-200 dark:border-slate-800">
        <span className="font-bold text-[11px] text-slate-700 dark:text-slate-200 uppercase tracking-wide">Consultas [{queries.length}]</span>
        <button
          data-testid="consultas-collapse"
          onClick={() => setCollapsed(true)}
          aria-label="Recolher painel de consultas"
          className="p-1 rounded hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500"
        >
          <ChevronsLeft className="w-3.5 h-3.5" />
        </button>
      </div>
      <div className="px-2 py-1.5 border-b border-slate-200 dark:border-slate-800 relative">
        <Search className="w-3 h-3 absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" />
        <input
          data-testid="consultas-search"
          placeholder="Pesquisar consultas"
          value={search}
          onChange={e => setSearch(e.target.value)}
          className="w-full pl-6 pr-2 py-1 text-[11px] rounded-md border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800"
        />
      </div>
      <div className="flex-1 overflow-y-auto py-1">
        {filtered.map(q => (
          <button
            key={q.id}
            data-testid={`consulta-item-${q.id}`}
            onClick={() => onSelect(q.id)}
            className={`w-full flex items-center gap-2 px-2 py-1.5 text-left text-[12px] ${
              q.id === selectedId
                ? 'bg-amber-100 dark:bg-amber-900/40 font-semibold text-slate-900 dark:text-slate-50'
                : 'hover:bg-slate-50 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300'
            }`}
          >
            <Table2 className="w-3.5 h-3.5 shrink-0 text-slate-400" />
            <span className="flex-1 truncate">{q.name}</span>
            <span className="shrink-0 text-[10px] px-1.5 py-0.5 rounded-full bg-slate-200 dark:bg-slate-700 text-slate-600 dark:text-slate-300 font-semibold">
              {q.stepCount}
            </span>
          </button>
        ))}
        {!filtered.length && <div className="px-3 py-2 text-[11px] text-slate-400">Nenhuma consulta encontrada</div>}
      </div>
    </div>
  );
};
