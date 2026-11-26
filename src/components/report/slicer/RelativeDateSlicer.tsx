import React from 'react';
import { RelativeDateFilter, RelativeDateUnit } from '../../../types/bi';
import { useTranslation } from '../../../i18n';
import { resolveRelativeDate } from '../../../engine/relativeDate';

const UNITS: RelativeDateUnit[] = ['days', 'weeks', 'months', 'quarters', 'years'];

/**
 * 'relative-date' slicer mode UI, matching Power BI's relative date slicer:
 * anchor (Últimos/Este/Próximos) + count + unit, with "incluir hoje" only
 * relevant for the `days` unit. The resolved [start, end] preview below the
 * controls is purely informational — `resolveRelativeDate` is re-run from
 * the stored definition (not these display dates) whenever the project is
 * reopened, so the filter always reflects the current day.
 */
export const RelativeDateSlicer: React.FC<{
  value: RelativeDateFilter;
  onChange: (value: RelativeDateFilter) => void;
}> = ({ value, onChange }) => {
  const { t } = useTranslation();
  const unitLabel: Record<RelativeDateUnit, string> = {
    days: t.slicer.unitDays,
    weeks: t.slicer.unitWeeks,
    months: t.slicer.unitMonths,
    quarters: t.slicer.unitQuarters,
    years: t.slicer.unitYears,
  };
  const preview = resolveRelativeDate(value, new Date());

  const selectCls =
    'bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded px-1.5 py-1 text-[11px] text-slate-800 dark:text-slate-100';

  return (
    <div className="flex flex-col gap-1.5" data-testid="relative-date-slicer">
      <div className="flex items-center gap-1.5 flex-wrap">
        <select
          aria-label="anchor"
          value={value.anchor}
          onChange={e => onChange({ ...value, anchor: e.target.value as RelativeDateFilter['anchor'] })}
          className={`${selectCls} min-w-0 flex-1`}
        >
          <option value="last">{t.slicer.anchorLast}</option>
          <option value="this">{t.slicer.anchorThis}</option>
          <option value="next">{t.slicer.anchorNext}</option>
        </select>
        <input
          type="number"
          min={1}
          aria-label="count"
          disabled={value.anchor === 'this'}
          value={value.count}
          onChange={e => onChange({ ...value, count: Math.max(1, Number(e.target.value) || 1) })}
          className={`${selectCls} w-16 shrink-0 disabled:opacity-50`}
        />
        <select
          aria-label="unit"
          value={value.unit}
          onChange={e => onChange({ ...value, unit: e.target.value as RelativeDateUnit })}
          className={`${selectCls} min-w-0 flex-1`}
        >
          {UNITS.map(u => (
            <option key={u} value={u}>
              {unitLabel[u]}
            </option>
          ))}
        </select>
      </div>
      {value.unit === 'days' && (
        <label className="flex items-center gap-1.5 text-[11px] text-slate-600 dark:text-slate-300">
          <input
            type="checkbox"
            checked={value.includeToday !== false}
            onChange={e => onChange({ ...value, includeToday: e.target.checked })}
            className="accent-amber-500 w-3 h-3"
          />
          {t.slicer.includeToday}
        </label>
      )}
      <div className="text-[10px] text-slate-400 dark:text-slate-500">
        {new Date(preview.start + 'T00:00:00').toLocaleDateString('pt-BR')} – {new Date(preview.end + 'T00:00:00').toLocaleDateString('pt-BR')}
      </div>
    </div>
  );
};
