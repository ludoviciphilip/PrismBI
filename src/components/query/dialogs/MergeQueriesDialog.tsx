import React, { useState } from 'react';
import { DialogShell, selectCls } from './DialogShell';

const JOIN_KINDS: { value: string; label: string }[] = [
  { value: 'leftOuter', label: 'Externa à esquerda (todas da esquerda)' },
  { value: 'inner', label: 'Interna (somente linhas correspondentes)' },
  { value: 'rightOuter', label: 'Externa à direita (todas da direita)' },
  { value: 'fullOuter', label: 'Externa completa (todas as linhas)' },
  { value: 'leftAnti', label: 'Anti à esquerda (somente da esquerda, sem correspondência)' },
  { value: 'rightAnti', label: 'Anti à direita (somente da direita, sem correspondência)' },
];

export interface MergeQueriesResult {
  rightQuery: string;
  leftKeys: string[];
  rightKeys: string[];
  joinKind: 'leftOuter' | 'inner' | 'rightOuter' | 'fullOuter' | 'leftAnti' | 'rightAnti';
  expand: { column: string; as: string }[];
}

export const MergeQueriesDialog: React.FC<{
  leftColumns: string[];
  availableTables: { id: string; name: string; columns: string[] }[];
  onClose: () => void;
  onApply: (result: MergeQueriesResult) => void;
}> = ({ leftColumns, availableTables, onClose, onApply }) => {
  const [rightTableName, setRightTableName] = useState(availableTables[0]?.name || '');
  const [leftKeys, setLeftKeys] = useState<string[]>([]);
  const [rightKeys, setRightKeys] = useState<string[]>([]);
  const [joinKind, setJoinKind] = useState<MergeQueriesResult['joinKind']>('leftOuter');
  const [expandCols, setExpandCols] = useState<string[]>([]);

  const rightTable = availableTables.find(t => t.name === rightTableName);
  const rightColumns = rightTable?.columns || [];

  const toggleLeftKey = (col: string) =>
    setLeftKeys(ks => (ks.includes(col) ? ks.filter(k => k !== col) : [...ks, col]));
  const toggleRightKey = (col: string) =>
    setRightKeys(ks => (ks.includes(col) ? ks.filter(k => k !== col) : [...ks, col]));
  const toggleExpand = (col: string) =>
    setExpandCols(cs => (cs.includes(col) ? cs.filter(c => c !== col) : [...cs, col]));

  const canApply = leftKeys.length > 0 && leftKeys.length === rightKeys.length && rightTableName;

  const handleApply = () => {
    onApply({
      rightQuery: rightTableName,
      leftKeys,
      rightKeys,
      joinKind,
      expand: expandCols.map(c => ({ column: c, as: `${rightTableName}.${c}` })),
    });
  };

  return (
    <DialogShell
      title="Mesclar Consultas"
      subtitle="Selecione as colunas-chave em cada tabela, na mesma ordem"
      onClose={onClose}
      onConfirm={handleApply}
      confirmLabel="Mesclar"
      confirmDisabled={!canApply}
      widthClass="max-w-3xl"
    >
      <div>
        <label className="font-semibold text-slate-600 dark:text-slate-300 block mb-1">Consulta da direita</label>
        <select
          value={rightTableName}
          onChange={e => {
            setRightTableName(e.target.value);
            setRightKeys([]);
            setExpandCols([]);
          }}
          className={`${selectCls} w-full`}
        >
          {availableTables.map(t => (
            <option key={t.id} value={t.name}>
              {t.name}
            </option>
          ))}
        </select>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div>
          <div className="font-semibold text-slate-600 dark:text-slate-300 mb-1">Chaves (esquerda)</div>
          <div className="border border-slate-200 dark:border-slate-700 rounded-lg max-h-40 overflow-y-auto" data-testid="merge-left-cols">
            {leftColumns.map(c => (
              <div
                key={c}
                onClick={() => toggleLeftKey(c)}
                data-testid={`left-col-${c}`}
                className={`px-2 py-1 cursor-pointer ${leftKeys.includes(c) ? 'bg-amber-100 dark:bg-amber-900/40 font-semibold' : 'hover:bg-slate-50 dark:hover:bg-slate-800'}`}
              >
                {leftKeys.includes(c) ? `${leftKeys.indexOf(c) + 1}. ` : ''}
                {c}
              </div>
            ))}
          </div>
        </div>
        <div>
          <div className="font-semibold text-slate-600 dark:text-slate-300 mb-1">Chaves (direita)</div>
          <div className="border border-slate-200 dark:border-slate-700 rounded-lg max-h-40 overflow-y-auto" data-testid="merge-right-cols">
            {rightColumns.map(c => (
              <div
                key={c}
                onClick={() => toggleRightKey(c)}
                data-testid={`right-col-${c}`}
                className={`px-2 py-1 cursor-pointer ${rightKeys.includes(c) ? 'bg-amber-100 dark:bg-amber-900/40 font-semibold' : 'hover:bg-slate-50 dark:hover:bg-slate-800'}`}
              >
                {rightKeys.includes(c) ? `${rightKeys.indexOf(c) + 1}. ` : ''}
                {c}
              </div>
            ))}
          </div>
        </div>
      </div>

      <div>
        <label className="font-semibold text-slate-600 dark:text-slate-300 block mb-1">Tipo de junção</label>
        <select value={joinKind} onChange={e => setJoinKind(e.target.value as any)} className={`${selectCls} w-full`}>
          {JOIN_KINDS.map(k => (
            <option key={k.value} value={k.value}>
              {k.label}
            </option>
          ))}
        </select>
      </div>

      <div>
        <div className="font-semibold text-slate-600 dark:text-slate-300 mb-1">Colunas a expandir da direita</div>
        <div className="flex flex-wrap gap-2" data-testid="merge-expand-cols">
          {rightColumns.map(c => (
            <label key={c} className="flex items-center gap-1 border border-slate-300 dark:border-slate-700 rounded-lg px-2 py-1">
              <input type="checkbox" checked={expandCols.includes(c)} onChange={() => toggleExpand(c)} />
              {c}
            </label>
          ))}
        </div>
      </div>
    </DialogShell>
  );
};
