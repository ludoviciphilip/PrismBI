import React, { useState } from 'react';
import { ChevronsLeft, ChevronsRight, Search, Table2, MoreVertical } from 'lucide-react';
import { ColumnContextMenu, ColumnMenuAction } from './ColumnContextMenu';

export interface QueryListEntry {
  id: string;
  name: string;
  stepCount: number;
  /** Tem alterações não aplicadas (ver useBiStore.isQueryDirty). */
  dirty?: boolean;
}

export interface ConsultaAction {
  label: string;
  onClick: () => void;
}

/**
 * Painel esquerdo "Consultas [N]": lista as consultas com ícone de tabela,
 * selo de nº de etapas, selo "alterações pendentes" e destaque da
 * selecionada, recolhível («) e com busca — substitui o <select> no canto
 * da ribbon. Clique direito (ou o botão "⋮") abre Renomear/Duplicar/
 * Referência/Excluir, como no Power Query Desktop.
 */
export const ConsultasPane: React.FC<{
  queries: QueryListEntry[];
  selectedId: string;
  onSelect: (id: string) => void;
  onRename?: (id: string) => void;
  onDuplicate?: (id: string) => void;
  onReference?: (id: string) => void;
  onDelete?: (id: string) => void;
}> = ({ queries, selectedId, onSelect, onRename, onDuplicate, onReference, onDelete }) => {
  const [collapsed, setCollapsed] = useState(false);
  const [search, setSearch] = useState('');
  const [menuFor, setMenuFor] = useState<string | null>(null);

  const filtered = queries.filter(q => q.name.toLowerCase().includes(search.toLowerCase()));
  const hasMenu = onRename || onDuplicate || onReference || onDelete;

  if (collapsed) {
    return (
      <div className="w-7 border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shrink-0 flex flex-col items-center py-2">
        <button
          data-testid="consultas-expand"
          onClick={() => setCollapsed(false)}
          aria-label="Expandir painel de consultas"
          className="p-1 rounded cursor-pointer hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500"
        >
          <ChevronsRight className="w-3.5 h-3.5" />
        </button>
      </div>
    );
  }

  const actionsFor = (id: string): ColumnMenuAction[] => {
    const list: ColumnMenuAction[] = [];
    if (onRename) list.push({ label: 'Renomear', onClick: () => onRename(id) });
    if (onDuplicate) list.push({ label: 'Duplicar', onClick: () => onDuplicate(id) });
    if (onReference) list.push({ label: 'Referência', onClick: () => onReference(id) });
    if (onDelete) list.push({ label: 'Excluir', onClick: () => onDelete(id) });
    return list;
  };

  return (
    <div className="w-56 border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shrink-0 flex flex-col" data-testid="consultas-pane">
      <div className="flex items-center justify-between px-2 py-1.5 border-b border-slate-200 dark:border-slate-800">
        <span className="font-bold text-[11px] text-slate-700 dark:text-slate-200 uppercase tracking-wide">Consultas [{queries.length}]</span>
        <button
          data-testid="consultas-collapse"
          onClick={() => setCollapsed(true)}
          aria-label="Recolher painel de consultas"
          className="p-1 rounded cursor-pointer hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500"
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
          <div
            key={q.id}
            className={`group relative flex items-center gap-2 px-2 py-1.5 text-[12px] ${
              q.id === selectedId
                ? 'bg-amber-100 dark:bg-amber-900/40'
                : 'hover:bg-slate-50 dark:hover:bg-slate-800'
            }`}
            onContextMenu={e => {
              if (!hasMenu) return;
              e.preventDefault();
              setMenuFor(q.id);
            }}
          >
            <button
              data-testid={`consulta-item-${q.id}`}
              onClick={() => onSelect(q.id)}
              className={`flex-1 flex items-center gap-2 text-left cursor-pointer min-w-0 ${
                q.id === selectedId
                  ? 'bg-amber-100 dark:bg-amber-900/40 font-semibold text-slate-900 dark:text-slate-50'
                  : 'text-slate-700 dark:text-slate-300'
              }`}
            >
              <Table2 className="w-3.5 h-3.5 shrink-0 text-slate-400" />
              <span className="flex-1 truncate">{q.name}</span>
              {q.dirty && (
                <span
                  data-testid={`consulta-dirty-${q.id}`}
                  title="Alterações pendentes (não aplicadas)"
                  className="shrink-0 w-1.5 h-1.5 rounded-full bg-amber-500"
                />
              )}
              <span className="shrink-0 text-[10px] px-1.5 py-0.5 rounded-full bg-slate-200 dark:bg-slate-700 text-slate-600 dark:text-slate-300 font-semibold">
                {q.stepCount}
              </span>
            </button>
            {hasMenu && (
              <button
                data-testid={`consulta-menu-btn-${q.id}`}
                onClick={e => {
                  e.stopPropagation();
                  setMenuFor(menuFor === q.id ? null : q.id);
                }}
                aria-label="Mais opções"
                className="shrink-0 p-0.5 rounded cursor-pointer opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-slate-200 dark:hover:bg-slate-700"
              >
                <MoreVertical className="w-3 h-3 text-slate-400" />
              </button>
            )}
            {menuFor === q.id && <ColumnContextMenu column={q.id} onClose={() => setMenuFor(null)} actions={actionsFor(q.id)} />}
          </div>
        ))}
        {!filtered.length && <div className="px-3 py-2 text-[11px] text-slate-400">Nenhuma consulta encontrada</div>}
      </div>
    </div>
  );
};
