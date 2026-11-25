import React, { useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ChevronRight } from 'lucide-react';
import { MenuNode, RibbonCtx, RibbonTabDef } from './menus';

/** Um botão do ribbon: ação direta ou abre um dropdown ▸ com submenus aninhados. */
const RibbonNodeButton: React.FC<{ node: MenuNode; ctx: RibbonCtx; onAction?: () => void }> = ({ node, ctx, onAction }) => {
  const [open, setOpen] = useState(false);
  const enabled = node.enabled ? node.enabled(ctx) : true;
  const title = !enabled ? node.disabledReason || 'Indisponível' : node.tooltip;
  const Icon = node.icon;

  if (node.kind === 'action') {
    return (
      <button
        data-testid={`ribbon-item-${node.id}`}
        title={title}
        disabled={!enabled}
        onClick={() => {
          node.run(ctx);
          onAction?.();
        }}
        className="w-full flex items-center gap-2 text-left px-3 py-1.5 text-[12px] rounded-md cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-800 active:bg-slate-100 dark:active:bg-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 focus-visible:ring-inset disabled:opacity-40 disabled:cursor-not-allowed"
      >
        {Icon && <Icon className="w-3.5 h-3.5 text-slate-400 shrink-0" />}
        <span>{node.label}</span>
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
        className="w-full flex items-center justify-between gap-2 px-3 py-1.5 text-[12px] rounded-md cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-800 active:bg-slate-100 dark:active:bg-slate-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 focus-visible:ring-inset disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <span className="flex items-center gap-2">
          {Icon && <Icon className="w-3.5 h-3.5 text-slate-400 shrink-0" />}
          {node.label}
        </span>
        <ChevronRight className="w-3 h-3 text-slate-400" data-testid={`ribbon-chevron-${node.id}`} />
      </button>
      {open && enabled && (
        // Submenu aninhado: já está dentro de um dropdown de topo (portalizado),
        // então pode ficar absolute normalmente — não sofre o corte do
        // `overflow-x-auto` da faixa de grupos (ver TopLevelButton).
        <div
          data-testid={`ribbon-submenu-${node.id}`}
          className="absolute left-full top-0 min-w-[200px] bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-xl py-1 z-50"
        >
          {node.items.map(child => (
            <RibbonNodeButton key={child.id} node={child} ctx={ctx} onAction={onAction} />
          ))}
        </div>
      )}
    </div>
  );
};

/**
 * Botão de topo-de-grupo: cada MenuNode de 1º nível vira um botão clicável/dropdown
 * na faixa. O dropdown é renderizado via portal em document.body e posicionado
 * pelas coordenadas do botão: a faixa de grupos usa `overflow-x-auto` para rolar
 * horizontalmente em 1280px, e por regra do CSS isso força `overflow-y` a
 * também cortar (computa para `auto`) — um dropdown `absolute` comum dentro
 * dela ficaria cortado verticalmente. O portal evita esse corte.
 */
const TopLevelButton: React.FC<{ node: MenuNode; ctx: RibbonCtx }> = ({ node, ctx }) => {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const btnRef = useRef<HTMLButtonElement>(null);
  const enabled = node.enabled ? node.enabled(ctx) : true;
  const title = !enabled ? node.disabledReason || 'Indisponível' : node.tooltip;
  const Icon = node.icon;
  const baseBtnClasses =
    'flex flex-col items-center justify-center gap-1 px-2.5 py-1.5 rounded-lg cursor-pointer min-w-[64px] max-w-[96px] ' +
    'hover:bg-slate-100 dark:hover:bg-slate-800 active:bg-slate-200 dark:active:bg-slate-700 ' +
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 ' +
    'disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent';

  useLayoutEffect(() => {
    if (open && btnRef.current) {
      const rect = btnRef.current.getBoundingClientRect();
      setPos({ left: rect.left, top: rect.bottom + 2 });
    }
  }, [open]);

  if (node.kind === 'action') {
    return (
      <button data-testid={`ribbon-item-${node.id}`} title={title} disabled={!enabled} onClick={() => node.run(ctx)} className={baseBtnClasses}>
        {Icon && <Icon className="w-5 h-5 text-slate-600 dark:text-slate-300" />}
        <span className="text-[10px] text-slate-600 dark:text-slate-300 leading-tight text-center">{node.label}</span>
      </button>
    );
  }

  return (
    <div
      className="relative"
      onMouseLeave={e => {
        // só fecha se o mouse não foi para o menu portalizado
        const next = e.relatedTarget as Node | null;
        if (!next || !(next as HTMLElement).closest?.(`[data-portal-for="${node.id}"]`)) setOpen(false);
      }}
    >
      <button
        ref={btnRef}
        data-testid={`ribbon-item-${node.id}`}
        title={title}
        disabled={!enabled}
        onClick={() => enabled && setOpen(o => !o)}
        onMouseEnter={() => enabled && setOpen(true)}
        className={baseBtnClasses}
      >
        {Icon && <Icon className="w-5 h-5 text-slate-600 dark:text-slate-300" />}
        <span className="text-[10px] text-slate-600 dark:text-slate-300 leading-tight text-center flex items-center gap-0.5">
          {node.label}
          <ChevronRight className="w-2.5 h-2.5 rotate-90" data-testid={`ribbon-chevron-${node.id}`} />
        </span>
      </button>
      {open && enabled && pos
        ? createPortal(
            <div
              data-testid={`ribbon-submenu-${node.id}`}
              data-portal-for={node.id}
              onMouseLeave={() => setOpen(false)}
              style={{ position: 'fixed', left: pos.left, top: pos.top }}
              className="min-w-[200px] bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-xl py-1 z-50"
            >
              {node.items.map(child => (
                <RibbonNodeButton key={child.id} node={child} ctx={ctx} onAction={() => setOpen(false)} />
              ))}
            </div>,
            document.body,
          )
        : null}
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
            className={`px-3 py-1.5 text-[12px] font-semibold rounded-t-lg cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400 ${
              tab.id === activeTab.id
                ? 'bg-slate-50 dark:bg-slate-950 text-amber-600 border border-b-0 border-slate-200 dark:border-slate-800'
                : 'text-slate-500 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-100'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div className="flex items-stretch gap-0 px-3 py-2 overflow-x-auto">
        {activeTab.groups.map((group, i) => (
          <div
            key={group.title}
            className={`flex flex-col shrink-0 px-2 ${i > 0 ? 'border-l border-slate-200 dark:border-slate-800' : ''}`}
          >
            <div className="flex items-start gap-1 flex-1">
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
