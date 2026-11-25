import { describe, it, expect } from 'vitest';
import { computeColumnDistribution } from '../distribution';

describe('computeColumnDistribution', () => {
  it('ranks the top values and counts distinct/unique', () => {
    const rows = [{ c: 'a' }, { c: 'a' }, { c: 'b' }, { c: 'c' }, { c: null }];
    const d = computeColumnDistribution(rows, 'c');
    expect(d.distinctCount).toBe(4); // a, b, c, (vazio)
    expect(d.uniqueCount).toBe(3); // b, c, (vazio) aparecem 1x
    expect(d.buckets[0]).toEqual({ value: 'a', count: 2, pct: 40 });
  });

  it('limits to topN buckets', () => {
    const rows = Array.from({ length: 20 }, (_, i) => ({ c: `v${i}` }));
    const d = computeColumnDistribution(rows, 'c', 5);
    expect(d.buckets).toHaveLength(5);
    expect(d.distinctCount).toBe(20);
  });

  it('returns no buckets for an empty row set', () => {
    const d = computeColumnDistribution([], 'c');
    expect(d.buckets).toEqual([]);
    expect(d.distinctCount).toBe(0);
  });
});
