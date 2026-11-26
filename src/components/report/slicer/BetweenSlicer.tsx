import React, { useEffect, useState } from 'react';
import { Column, Table } from '../../../types/bi';
import { useTranslation } from '../../../i18n';
import { getColumnRange } from './useColumnRange';

export interface BetweenValue {
  start?: string | number;
  end?: string | number;
}

/**
 * 'between' slicer mode UI: two date inputs + a range slider for date
 * columns, or min/max number inputs + a range slider for numeric columns.
 * The slider's own min/max come from the column's real range in the model
 * (via `getColumnRange`, which queries the native DuckDB table rather than
 * the 5.000-row `table.rows` preview), not from the preview rows.
 */
export const BetweenSlicer: React.FC<{
  table: Table;
  column: Column;
  value: BetweenValue;
  onChange: (value: BetweenValue) => void;
}> = ({ table, column, value, onChange }) => {
  const { t } = useTranslation();
  const isDate = column.dataType === 'date';
  const [range, setRange] = useState<{ min: number | string; max: number | string } | null>(null);

  useEffect(() => {
    let cancelled = false;
    getColumnRange(table, column.name).then(r => {
      if (!cancelled) setRange(r);
    });
    return () => {
      cancelled = true;
    };
  }, [table.id, column.name]);

  const minBound = range ? Number(isDate ? new Date(String(range.min)).getTime() : range.min) : 0;
  const maxBound = range ? Number(isDate ? new Date(String(range.max)).getTime() : range.max) : 100;

  const startNum = value.start !== undefined ? Number(isDate ? new Date(String(value.start)).getTime() : value.start) : minBound;
  const endNum = value.end !== undefined ? Number(isDate ? new Date(String(value.end)).getTime() : value.end) : maxBound;

  const toValue = (n: number): string | number => (isDate ? new Date(n).toISOString().slice(0, 10) : n);

  const inputCls =
    'min-w-0 flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-[11px] text-slate-800 dark:text-slate-100';

  return (
    <div className="flex flex-col gap-1.5" data-testid="between-slicer">
      <div className="flex items-center gap-1.5">
        <label className="text-[10px] text-slate-500 dark:text-slate-400 w-10 shrink-0">
          {isDate ? t.slicer.start : t.slicer.min}
        </label>
        <input
          type={isDate ? 'date' : 'number'}
          aria-label={isDate ? t.slicer.start : t.slicer.min}
          value={value.start ?? (range ? range.min : '')}
          min={isDate ? undefined : range?.min}
          max={isDate ? undefined : range?.max}
          onChange={e => onChange({ ...value, start: isDate ? e.target.value : Number(e.target.value) })}
          className={inputCls}
        />
      </div>
      <div className="flex items-center gap-1.5">
        <label className="text-[10px] text-slate-500 dark:text-slate-400 w-10 shrink-0">
          {isDate ? t.slicer.end : t.slicer.max}
        </label>
        <input
          type={isDate ? 'date' : 'number'}
          aria-label={isDate ? t.slicer.end : t.slicer.max}
          value={value.end ?? (range ? range.max : '')}
          min={isDate ? undefined : range?.min}
          max={isDate ? undefined : range?.max}
          onChange={e => onChange({ ...value, end: isDate ? e.target.value : Number(e.target.value) })}
          className={inputCls}
        />
      </div>
      {range && minBound < maxBound && (
        <div className="flex items-center gap-1.5 pt-0.5">
          <input
            type="range"
            aria-label={t.slicer.start}
            min={minBound}
            max={maxBound}
            value={startNum}
            onChange={e => onChange({ ...value, start: toValue(Math.min(Number(e.target.value), endNum)) })}
            className="flex-1 accent-amber-500"
          />
          <input
            type="range"
            aria-label={t.slicer.end}
            min={minBound}
            max={maxBound}
            value={endNum}
            onChange={e => onChange({ ...value, end: toValue(Math.max(Number(e.target.value), startNum)) })}
            className="flex-1 accent-amber-500"
          />
        </div>
      )}
    </div>
  );
};
