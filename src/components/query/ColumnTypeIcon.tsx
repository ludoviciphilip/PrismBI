import React from 'react';
import { Type, Hash, Calendar, ToggleLeft, DollarSign, Clock, HelpCircle } from 'lucide-react';
import { EtlType } from '../../types/etl';

export function etlTypeFromDuckType(duckType: string): EtlType {
  const t = (duckType || '').toLowerCase();
  if (t.includes('bool')) return 'boolean';
  if (t.includes('timestamp') || t.includes('datetime')) return 'datetime';
  if (t.includes('date')) return 'date';
  if (t.includes('decimal') || t.includes('double') || t.includes('float') || t.includes('real')) return 'decimal';
  if (t.includes('int') || t.includes('bigint') || t.includes('smallint')) return 'integer';
  return 'text';
}

export const ColumnTypeIcon: React.FC<{ type: string; className?: string }> = ({ type, className }) => {
  const etlType = (['text', 'integer', 'decimal', 'currency', 'date', 'datetime', 'boolean'].includes(type)
    ? (type as EtlType)
    : etlTypeFromDuckType(type));
  const cls = className || 'w-3 h-3';
  switch (etlType) {
    case 'integer':
    case 'decimal':
      return <Hash className={cls} />;
    case 'currency':
      return <DollarSign className={cls} />;
    case 'date':
      return <Calendar className={cls} />;
    case 'datetime':
      return <Clock className={cls} />;
    case 'boolean':
      return <ToggleLeft className={cls} />;
    case 'text':
      return <Type className={cls} />;
    default:
      return <HelpCircle className={cls} />;
  }
};
