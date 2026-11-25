import React from 'react';

export interface ColumnMenuAction {
  label: string;
  onClick: () => void;
  submenu?: { label: string; onClick: () => void }[];
}

export const ColumnContextMenu: React.FC<{
  column: string;
  onClose: () => void;
  actions: ColumnMenuAction[];
}> = ({ column, onClose, actions }) => {
  const [openSub, setOpenSub] = React.useState<number | null>(null);
  return (
    <div
      data-testid={`column-context-menu-${column}`}
      className="absolute z-40 mt-1 w-56 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-xl py-1 text-[12px]"
      onMouseLeave={() => setOpenSub(null)}
    >
      {actions.map((a, i) => (
        <div key={i} className="relative">
          <button
            onClick={() => {
              if (a.submenu) {
                setOpenSub(openSub === i ? null : i);
              } else {
                a.onClick();
                onClose();
              }
            }}
            className="w-full text-left px-3 py-1.5 hover:bg-slate-50 dark:hover:bg-slate-800 flex items-center justify-between"
          >
            {a.label}
            {a.submenu && <span>▸</span>}
          </button>
          {a.submenu && openSub === i && (
            <div className="absolute left-full top-0 w-48 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-xl py-1">
              {a.submenu.map((s, j) => (
                <button
                  key={j}
                  onClick={() => {
                    s.onClick();
                    onClose();
                  }}
                  className="w-full text-left px-3 py-1.5 hover:bg-slate-50 dark:hover:bg-slate-800"
                >
                  {s.label}
                </button>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
};
