import { RelativeDateFilter, RelativeDateUnit } from '../types/bi';

/**
 * Resolves a Power BI-style "relative date" slicer definition to a concrete
 * [start, end] (inclusive) ISO date range (`YYYY-MM-DD`, no time component),
 * anchored on a given `today`.
 *
 * Semantics (matches Power BI Desktop's relative date slicer):
 * - `days` is the only unit with no calendar grouping — "últimos/próximos N
 *   dias" is a rolling N-day window anchored on `today`. `includeToday`
 *   decides whether `today` itself is inside the window (default: true).
 *   "this" + days has no Power BI equivalent and simply resolves to today.
 * - `weeks` / `months` / `quarters` / `years` are CALENDAR periods:
 *   - `this`: the current period in full, start to end (e.g. "este mês" is
 *     the 1st to the last day of the current month, regardless of what day
 *     of the month `today` is).
 *   - `last N`: the N complete periods immediately BEFORE the current one.
 *     The current (partial) period is never included, so "últimos 3 meses"
 *     evaluated on any day of December is Sep 1 – Nov 30, not part of
 *     December.
 *   - `next N`: the N complete periods immediately AFTER the current one.
 *     Mirrors `last`: the current period is never included.
 * - Weeks start on SUNDAY (Power BI's en-US default week start). A
 *   different locale would start on Monday, but this project targets pt-BR
 *   reports built against Power BI Desktop's en-US default, so Sunday is
 *   used consistently and documented here rather than made configurable.
 * - Quarters are calendar quarters (Jan–Mar, Apr–Jun, Jul–Sep, Oct–Dec).
 *
 * All arithmetic is done on UTC-normalized dates (local Y/M/D, re-anchored
 * at UTC midnight) purely to avoid DST drift when adding days; the calendar
 * math (month/quarter/year lengths, leap years) is otherwise exactly the
 * Gregorian calendar.
 */
export interface ResolvedDateRange {
  start: string;
  end: string;
}

function toUTCDate(y: number, m: number, d: number): Date {
  return new Date(Date.UTC(y, m, d));
}

/** Normalizes any `Date` (or date-like) to a UTC-midnight date carrying only its local Y/M/D. */
function normalizeToday(today: Date): Date {
  return toUTCDate(today.getFullYear(), today.getMonth(), today.getDate());
}

function addDays(d: Date, days: number): Date {
  return new Date(d.getTime() + days * 86400000);
}

function addMonths(d: Date, months: number): Date {
  return toUTCDate(d.getUTCFullYear(), d.getUTCMonth() + months, d.getUTCDate());
}

function toISO(d: Date): string {
  return d.toISOString().slice(0, 10);
}

/** Sunday-anchored start of the week containing `d` (Power BI en-US default). */
function weekStart(d: Date): Date {
  const dow = d.getUTCDay(); // 0 = Sunday
  return addDays(d, -dow);
}

/** First day of the calendar month containing `d`. */
function monthStart(d: Date): Date {
  return toUTCDate(d.getUTCFullYear(), d.getUTCMonth(), 1);
}

/** First day of the calendar quarter containing `d`. */
function quarterStart(d: Date): Date {
  const qMonth = Math.floor(d.getUTCMonth() / 3) * 3;
  return toUTCDate(d.getUTCFullYear(), qMonth, 1);
}

/** First day of the calendar year containing `d`. */
function yearStart(d: Date): Date {
  return toUTCDate(d.getUTCFullYear(), 0, 1);
}

interface PeriodOps {
  /** Start of the period (e.g. week/month/quarter/year) containing `d`. */
  start: (d: Date) => Date;
  /** Start of the period `n` periods away from `periodStart` (n may be negative). */
  advance: (periodStart: Date, n: number) => Date;
}

const PERIOD_OPS: Record<Exclude<RelativeDateUnit, 'days'>, PeriodOps> = {
  weeks: { start: weekStart, advance: (s, n) => addDays(s, n * 7) },
  months: { start: monthStart, advance: (s, n) => addMonths(s, n) },
  quarters: { start: quarterStart, advance: (s, n) => addMonths(s, n * 3) },
  years: { start: yearStart, advance: (s, n) => addMonths(s, n * 12) },
};

export function resolveRelativeDate(filter: RelativeDateFilter, today: Date = new Date()): ResolvedDateRange {
  const t = normalizeToday(today);
  const count = Math.max(1, Math.floor(filter.count) || 1);

  if (filter.unit === 'days') {
    const includeToday = filter.includeToday !== false;
    if (filter.anchor === 'this') {
      // "This X days" has no Power BI equivalent; resolves to today only.
      return { start: toISO(t), end: toISO(t) };
    }
    if (filter.anchor === 'last') {
      const end = includeToday ? t : addDays(t, -1);
      const start = addDays(end, -(count - 1));
      return { start: toISO(start), end: toISO(end) };
    }
    // 'next'
    const start = includeToday ? t : addDays(t, 1);
    const end = addDays(start, count - 1);
    return { start: toISO(start), end: toISO(end) };
  }

  const ops = PERIOD_OPS[filter.unit];
  const currentStart = ops.start(t);

  if (filter.anchor === 'this') {
    const nextStart = ops.advance(currentStart, 1);
    return { start: toISO(currentStart), end: toISO(addDays(nextStart, -1)) };
  }
  if (filter.anchor === 'last') {
    const start = ops.advance(currentStart, -count);
    const end = addDays(currentStart, -1);
    return { start: toISO(start), end: toISO(end) };
  }
  // 'next'
  const start = ops.advance(currentStart, 1);
  const end = addDays(ops.advance(start, count), -1);
  return { start: toISO(start), end: toISO(end) };
}
