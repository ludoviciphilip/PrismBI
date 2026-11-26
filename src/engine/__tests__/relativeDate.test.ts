import { describe, it, expect } from 'vitest';
import { resolveRelativeDate } from '../relativeDate';

// Hand-derived expected values — see relativeDate.ts doc comment for the
// semantics (days = rolling window; weeks/months/quarters/years = complete
// calendar periods; weeks start on Sunday; 'last'/'next' never include the
// current period).
describe('resolveRelativeDate', () => {
  const d = (s: string) => new Date(`${s}T12:00:00`); // local noon, avoids any DST edge at midnight

  it('last N days includes today when includeToday=true', () => {
    // 2024-03-10 is a Sunday.
    expect(resolveRelativeDate({ anchor: 'last', count: 7, unit: 'days', includeToday: true }, d('2024-03-10'))).toEqual({
      start: '2024-03-04',
      end: '2024-03-10',
    });
  });

  it('last N days excludes today when includeToday=false', () => {
    expect(resolveRelativeDate({ anchor: 'last', count: 7, unit: 'days', includeToday: false }, d('2024-03-10'))).toEqual({
      start: '2024-03-03',
      end: '2024-03-09',
    });
  });

  it('next N days includes today when includeToday=true', () => {
    expect(resolveRelativeDate({ anchor: 'next', count: 5, unit: 'days', includeToday: true }, d('2024-03-10'))).toEqual({
      start: '2024-03-10',
      end: '2024-03-14',
    });
  });

  it('next N days excludes today when includeToday=false', () => {
    expect(resolveRelativeDate({ anchor: 'next', count: 5, unit: 'days', includeToday: false }, d('2024-03-10'))).toEqual({
      start: '2024-03-11',
      end: '2024-03-15',
    });
  });

  it('this (days) resolves to today only', () => {
    expect(resolveRelativeDate({ anchor: 'this', count: 1, unit: 'days' }, d('2024-03-10'))).toEqual({
      start: '2024-03-10',
      end: '2024-03-10',
    });
  });

  it('this week runs Sunday to Saturday', () => {
    // 2024-03-13 is a Wednesday; its week is Sun 2024-03-10 .. Sat 2024-03-16.
    expect(resolveRelativeDate({ anchor: 'this', count: 1, unit: 'weeks' }, d('2024-03-13'))).toEqual({
      start: '2024-03-10',
      end: '2024-03-16',
    });
  });

  it('last 2 weeks excludes the current (partial) week', () => {
    // Current week starts Sun 2024-03-10. Last 2 complete weeks: Feb 25 .. Mar 9.
    expect(resolveRelativeDate({ anchor: 'last', count: 2, unit: 'weeks' }, d('2024-03-13'))).toEqual({
      start: '2024-02-25',
      end: '2024-03-09',
    });
  });

  it('this month is the whole current month (leap February)', () => {
    expect(resolveRelativeDate({ anchor: 'this', count: 1, unit: 'months' }, d('2024-02-15'))).toEqual({
      start: '2024-02-01',
      end: '2024-02-29',
    });
  });

  it('this month is the whole current month (non-leap February)', () => {
    expect(resolveRelativeDate({ anchor: 'this', count: 1, unit: 'months' }, d('2023-02-15'))).toEqual({
      start: '2023-02-01',
      end: '2023-02-28',
    });
  });

  it('last 3 months excludes the current month and crosses a year boundary', () => {
    // Matches the FiltersPane example in docs: today in December 2026 -> Sep 1 .. Nov 30, 2026.
    expect(resolveRelativeDate({ anchor: 'last', count: 3, unit: 'months' }, d('2026-12-15'))).toEqual({
      start: '2026-09-01',
      end: '2026-11-30',
    });
  });

  it('next 2 months are complete calendar months and cross a year boundary', () => {
    expect(resolveRelativeDate({ anchor: 'next', count: 2, unit: 'months' }, d('2024-12-05'))).toEqual({
      start: '2025-01-01',
      end: '2025-02-28',
    });
  });

  it('this quarter runs the full calendar quarter', () => {
    // 2024-05-20 is in Q2 (Apr-Jun).
    expect(resolveRelativeDate({ anchor: 'this', count: 1, unit: 'quarters' }, d('2024-05-20'))).toEqual({
      start: '2024-04-01',
      end: '2024-06-30',
    });
  });

  it('next 1 quarter', () => {
    expect(resolveRelativeDate({ anchor: 'next', count: 1, unit: 'quarters' }, d('2024-05-20'))).toEqual({
      start: '2024-07-01',
      end: '2024-09-30',
    });
  });

  it('this year is the full calendar year', () => {
    expect(resolveRelativeDate({ anchor: 'this', count: 1, unit: 'years' }, d('2024-06-01'))).toEqual({
      start: '2024-01-01',
      end: '2024-12-31',
    });
  });

  it('last 1 year excludes the current year', () => {
    expect(resolveRelativeDate({ anchor: 'last', count: 1, unit: 'years' }, d('2025-06-01'))).toEqual({
      start: '2024-01-01',
      end: '2024-12-31',
    });
  });

  it('next 1 year crossing into a leap year', () => {
    expect(resolveRelativeDate({ anchor: 'next', count: 1, unit: 'years' }, d('2023-06-01'))).toEqual({
      start: '2024-01-01',
      end: '2024-12-31',
    });
  });
});
