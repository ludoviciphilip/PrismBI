import { describe, it, expect } from 'vitest';
import { computeColumnQuality } from '../quality';

describe('computeColumnQuality', () => {
  it('counts valid/error/empty over the preview rows', () => {
    const rows = [
      { c: '1' },
      { c: '2', __err__c: true },
      { c: '' },
      { c: null },
      { c: '5' },
    ];
    const q = computeColumnQuality(rows, 'c');
    expect(q.total).toBe(5);
    expect(q.error).toBe(1);
    expect(q.empty).toBe(2);
    expect(q.valid).toBe(2);
    expect(q.validPct).toBeCloseTo(40);
    expect(q.errorPct).toBeCloseTo(20);
    expect(q.emptyPct).toBeCloseTo(40);
  });

  it('treats a missing __err__ flag as "no error" (fallback when preview has no flags)', () => {
    const rows = [{ c: 'a' }, { c: 'b' }];
    const q = computeColumnQuality(rows, 'c');
    expect(q.error).toBe(0);
    expect(q.valid).toBe(2);
  });

  it('only profiles the first 200 rows', () => {
    const rows = Array.from({ length: 300 }, (_, i) => ({ c: i < 200 ? 'x' : '' }));
    const q = computeColumnQuality(rows, 'c');
    expect(q.total).toBe(200);
    expect(q.valid).toBe(200);
    expect(q.empty).toBe(0);
  });

  it('handles an empty row set', () => {
    const q = computeColumnQuality([], 'c');
    expect(q).toEqual({ total: 0, valid: 0, error: 0, empty: 0, validPct: 0, errorPct: 0, emptyPct: 0 });
  });
});
