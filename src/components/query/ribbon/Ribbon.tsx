import React, { useState } from 'react';
import { ChevronRight } from 'lucide-react';
import { MenuNode, RibbonCtx, RibbonTabDef } from './menus';

/** Um botão do ribbon: ação direta ou abre um dropdown ▸ com submenus aninhados. */
const RibbonNodeButton: React.FC<{ node: MenuNode; ctx: RibbonCtx }> = ({ node, ctx }) => {
  const [open, setOpen] = useState(false);
  const enabled = node.enabled ? node.enabled(ctx) : true;
  const title = !enabled ? node.disabledReason || 'Indisponível' : undefined;

  if (node.kind === 'action') {
    return (
      <button
        data-testid={`ribbon-item-${node.id}`}
        title={title}
        disabled={!enabled}
        onClick={() => node.run(ctx)}
        className="w-full text-left px-3 py-1.5 text-[12px] hover:bg-slate-50 dark:hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed rounded-md"
      >
        {node.label}
      </button>
    );
  }

  return (
    <div className="relative" onMouseLeave={() => setOpen(false)}>
      <button
        data-testid={`ribbon-item-${node.id}`}
        title={title}
        disabled={!enabled}
        onClick={() => enabled && setOpen(o => !o)}
        onMouseEnter={() => enabled && setOpen(true)}
        className="w-full flex items-center justify-between gap-2 px-3 py-1.5 text-[12px] hover:bg-slate-50 dark:hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed rounded-md"
      >
        <span>{node.label}</span>
        <ChevronRight className="w-3 h-3 text-slate-400" />
      </button>
      {open && enabled && (
        <div
          data-testid={`ribbon-submenu-${node.id}`}
          className="absolute left-full top-0 min-w-[200px] bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-xl py-1 z-50"
        >
          {node.items.map(child => (
            <RibbonNodeButton key={child.id} node={child} ctx={ctx} />
          ))}
        </div>
      )}
    </div>
  );
};

/** Botão de topo-de-grupo: cada MenuNode de 1º nível vira um botão clicável/dropdown na faixa. */
const TopLevelButton: React.FC<{ node: MenuNode; ctx: RibbonCtx }> = ({ node, ctx }) => {
  const [open, setOpen] = useState(false);
  const enabled = node.enabled ? node.enabled(ctx) : true;
  const title = !enabled ? node.disabledReason || 'Indisponível' : undefined;

  if (node.kind === 'action') {
    return (
      <button
        data-testid={`ribbon-item-${node.id}`}
        title={title}
        disabled={!enabled}
        onClick={() => node.run(ctx)}
        className="flex flex-col items-center justify-center gap-0.5 px-2 py-1 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed min-w-[64px] max-w-[96px]"
      >
        <span className="text-[10px] text-slate-600 dark:text-slate-300 leading-tight text-center">{node.label}</span>
      </button>
    );
  }

  return (
    <div className="relative" onMouseLeave={() => setOpen(false)}>
      <button
        data-testid={`ribbon-item-${node.id}`}
        title={title}
        disabled={!enabled}
        onClick={() => enabled && setOpen(o => !o)}
        onMouseEnter={() => enabled && setOpen(true)}
        className="flex flex-col items-center justify-center gap-0.5 px-2 py-1 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-30 disabled:cursor-not-allowed min-w-[64px] max-w-[96px]"
      >
        <span className="text-[10px] text-slate-600 dark:text-slate-300 leading-tight text-center flex items-center gap-0.5">
          {node.label}
          <ChevronRight className="w-2.5 h-2.5 rotate-90" />
        </span>
      </button>
      {open && enabled && (
        <div
          data-testid={`ribbon-submenu-${node.id}`}
          className="absolute left-0 top-full mt-0.5 min-w-[200px] bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-xl py-1 z-50"
        >
          {node.items.map(child => (
            <RibbonNodeButton key={child.id} node={child} ctx={ctx} />
          ))}
        </div>
      )}
    </div>
  );
};

/**
 * A faixa de opções em abas (Página Inicial | Transformar | Adicionar Coluna |
 * Exibir), com grupos rotulados e botões/dropdowns data-driven. Em 1280px os
 * grupos ficam em `overflow-x-auto` por faixa, nunca sobrepõem.
 */
export const Ribbon: React.FC<{
  tabs: RibbonTabDef[];
  activeTabId: string;
  onTabChange: (id: string) => void;
  ctx: RibbonCtx;
}> = ({ tabs, activeTabId, onTabChange, ctx }) => {
  const activeTab = tabs.find(t => t.id === activeTabId) || tabs[0];
  return (
    <div className="border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900" data-testid="ribbon">
      <div className="flex items-center gap-1 px-3 pt-1.5 border-b border-slate-100 dark:border-slate-800">
        {tabs.map(tab => (
          <button
            key={tab.id}
            data-testid={`ribbon-tab-${tab.id}`}
            onClick={() => onTabChange(tab.id)}
            className={`px-3 py-1.5 text-[12px] font-semibold rounded-t-lg ${
              tab.id === activeTab.id
                ? 'bg-slate-50 dark:bg-slate-950 text-amber-600 border border-b-0 border-slate-200 dark:border-slate-800'
                : 'text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-100'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div className="flex items-start gap-4 px-3 py-2 overflow-x-auto">
        {activeTab.groups.map(group => (
          <div key={group.title} className="flex flex-col shrink-0">
            <div className="flex items-start gap-1">
              {group.items.map(node => (
                <TopLevelButton key={node.id} node={node} ctx={ctx} />
              ))}
            </div>
            <div className="text-center text-[10px] text-slate-400 mt-1 border-t border-slate-100 dark:border-slate-800 pt-1">{group.title}</div>
          </div>
        ))}
      </div>
    </div>
  );
};
