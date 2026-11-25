import { describe, it, expect } from 'vitest';
import { computeColumnProfile } from '../profile';

describe('computeColumnProfile', () => {
  it('computes numeric stats (min/max/mean/stdDev) for number columns', () => {
    const rows = [{ c: 1 }, { c: 2 }, { c: 3 }, { c: 4 }];
    const p = computeColumnProfile(rows, 'c', 'integer');
    expect(p.min).toBe(1);
    expect(p.max).toBe(4);
    expect(p.mean).toBe(2.5);
    expect(p.stdDev).toBeCloseTo(1.12, 1);
    expect(p.count).toBe(4);
  });

  it('computes text stats (empty strings / min / max length) for text columns', () => {
    const rows = [{ c: 'abc' }, { c: 'de' }, { c: '' }, { c: null }];
    const p = computeColumnProfile(rows, 'c', 'text');
    expect(p.emptyStrings).toBe(1);
    expect(p.minLength).toBe(2);
    expect(p.maxLength).toBe(3);
    expect(p.empty).toBe(2); // '' e null
  });

  it('carries distinct/unique from the distribution', () => {
    const rows = [{ c: 'a' }, { c: 'a' }, { c: 'b' }];
    const p = computeColumnProfile(rows, 'c', 'text');
    expect(p.distinct).toBe(2);
    expect(p.unique).toBe(1);
  });

  it('computes min/max DATE (not string length) for date columns', () => {
    const rows = [{ c: '2026-03-10' }, { c: '2026-01-05' }, { c: '2026-06-20' }, { c: null }];
    const p = computeColumnProfile(rows, 'c', 'date');
    expect(p.minDate).toBe('2026-01-05');
    expect(p.maxDate).toBe('2026-06-20');
    expect(p.minLength).toBeUndefined();
    expect(p.maxLength).toBeUndefined();
    expect(p.empty).toBe(1);
  });

  it('computes min/max DATETIME the same way', () => {
    const rows = [{ c: '2026-03-10T08:00:00' }, { c: '2026-03-10T23:00:00' }];
    const p = computeColumnProfile(rows, 'c', 'datetime');
    expect(p.minDate).toBe('2026-03-10T08:00:00');
    expect(p.maxDate).toBe('2026-03-10T23:00:00');
  });
});
