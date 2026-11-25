import React from 'react';
import { ColumnDistribution } from './distribution';

/** Mini histograma dos valores mais comuns + "N distintos, M únicos". */
export const DistributionBar: React.FC<{ distribution: ColumnDistribution }> = ({ distribution }) => {
  const max = Math.max(1, ...distribution.buckets.map(b => b.count));
  return (
    <div data-testid="distribution-bar" className="mt-1">
      <div className="flex items-end gap-px h-5">
        {distribution.buckets.map(b => (
          <div key={b.value} title={`${b.value}: ${b.count}`} className="bg-sky-400 dark:bg-sky-600 flex-1" style={{ height: `${(b.count / max) * 100}%` }} />
        ))}
      </div>
      <div className="text-[9px] text-slate-400 mt-0.5 truncate">
        {distribution.distinctCount} distintos, {distribution.uniqueCount} únicos
      </div>
    </div>
  );
};
