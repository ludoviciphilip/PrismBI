import React from 'react';
import { X } from 'lucide-react';
import { Table, Visual, VisualDataConfig } from '../../../types/bi';
import { makeColumnTypeLookup } from './useVisualDrill';
import { useTranslation } from '../../../i18n';

const SELECT_CLASS =
  'flex-1 min-w-0 w-full bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-[12px] ' +
  'text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-amber-500 disabled:opacity-40 disabled:cursor-not-allowed';

export interface DrillLevelsEditorProps {
  visual: Visual;
  table: Table | undefined;
  relatedTables: Table[];
  onChange: (patch: Partial<VisualDataConfig>) => void;
}

/**
 * Painel Visualizações: níveis da hierarquia. Matriz → `matrixRowFields` (N níveis
 * nas linhas, o 1º é o campo "Linhas"); gráficos → `drillFields` (níveis abaixo do
 * eixo). Para eixo de data, liga/desliga a hierarquia automática Ano › Trimestre ›
 * Mês › Dia.
 */
export const DrillLevelsEditor: React.FC<DrillLevelsEditorProps> = ({ visual, table, relatedTables, onChange }) => {
  const { t } = useTranslation();
  const de = t.drillEditor;
  const dc = visual.dataConfig;
  const isMatrix = visual.type === 'matrix';
  const x = dc.xAxisField;
  const stored = dc.matrixRowFields || [];
  const subs: string[] = isMatrix
    ? stored.length > 1
      ? stored.slice(1)
      : (dc.drillFields || []).slice(0, 1)
    : dc.drillFields || [];

  const options: { value: string; label: string }[] = [
    ...(table?.columns || []).map(c => ({ value: c.name, label: c.name })),
    ...relatedTables.flatMap(t2 => t2.columns.map(c => ({ value: `${t2.name}.${c.name}`, label: `${t2.name}.${c.name}` }))),
  ];
  const typeOf = makeColumnTypeLookup([...(table ? [table] : []), ...relatedTables], dc.tableId);
  const isDateAxis = Boolean(x) && typeOf(x || '') === 'date';
  const autoDate = dc.drillFields === undefined && dc.drillMode !== undefined;

  const setSubs = (next: string[]) => {
    const clean = next.filter(Boolean);
    if (isMatrix) {
      onChange(
        clean.length
          ? { matrixRowFields: [x as string, ...clean], drillFields: clean }
          : { matrixRowFields: undefined, drillFields: undefined }
      );
    } else {
      onChange({ drillFields: clean.length ? clean : undefined });
    }
  };

  const noAxisHint = isMatrix ? de.chooseAxisFirstMatrix : de.chooseAxisFirstChart;
  const used = new Set([x, ...subs]);

  return (
    <div data-testid="drill-levels-editor">
      <label className="text-[11px] font-semibold text-slate-600 dark:text-slate-400 uppercase tracking-wider block mb-1">
        {isMatrix ? de.rowLevelsLabel : de.axisHierarchyLabel}
      </label>
      <div className="flex flex-col gap-1">
        {subs.map((field, i) => (
          <div key={`${i}:${field}`} className="flex items-center gap-1">
            <span className="w-12 shrink-0 text-[10px] text-slate-500 dark:text-slate-400">{de.level.replace('{n}', String(i + 2))}</span>
            <select
              aria-label={de.level.replace('{n}', String(i + 2))}
              value={field}
              onChange={e => setSubs(subs.map((f, j) => (j === i ? e.target.value : f)))}
              className={SELECT_CLASS}
            >
              {options.map(o => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
            <button
              type="button"
              aria-label={de.removeLevel.replace('{n}', String(i + 2))}
              title={de.removeLevel.replace('{n}', String(i + 2))}
              onClick={() => setSubs(subs.filter((_, j) => j !== i))}
              className="w-6 h-6 shrink-0 inline-flex items-center justify-center rounded text-slate-400 hover:text-red-600 hover:bg-slate-200/60 dark:hover:bg-slate-700/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        ))}
        <select
          aria-label={de.addLevelAria}
          value=""
          disabled={!x}
          title={x ? de.addLevelTitle : noAxisHint}
          onChange={e => e.target.value && setSubs([...subs, e.target.value])}
          className={SELECT_CLASS}
        >
          <option value="">{de.addLevelOption}</option>
          {options
            .filter(o => !used.has(o.value))
            .map(o => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
        </select>
        {isDateAxis && subs.length === 0 && (
          <label className="flex items-center gap-1.5 text-[11px] text-slate-600 dark:text-slate-300 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={autoDate}
              onChange={e =>
                onChange(
                  e.target.checked
                    ? { drillFields: undefined, drillMode: dc.drillMode ?? 'drill', ...(isMatrix ? { matrixRowFields: undefined } : {}) }
                    : { drillFields: [] }
                )
              }
              className="accent-amber-500"
            />
            {de.autoDateHierarchy}
          </label>
        )}
      </div>
    </div>
  );
};
