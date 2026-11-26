/**
 * Célula de tabela/matriz com suporte a barra de dados (fundo em degrade
 * horizontal partindo de um eixo, negativos crescendo para a esquerda) e
 * ícone condicional inline. Usado por VisualCard.tsx nas renderizações de
 * tabela e matriz (ganchos mínimos).
 */
import React from 'react';
import type { CSSProperties, ReactNode } from 'react';
import type { FormatResult } from '../../../engine/conditionalFormat';
import { ConditionalIcon } from './icons';

export function FormattedCell({
  format,
  className,
  style,
  children,
  onClick,
  title,
}: {
  format: FormatResult;
  className?: string;
  style?: CSSProperties;
  children: ReactNode;
  onClick?: () => void;
  title?: string;
}) {
  const cellStyle: CSSProperties = {
    ...style,
    position: 'relative',
    backgroundColor: format.background ?? style?.backgroundColor,
    color: format.color ?? style?.color,
  };

  return (
    <td className={className} style={cellStyle} onClick={onClick} title={title}>
      {format.dataBar && (
        <div
          aria-hidden
          style={{
            position: 'absolute',
            inset: 0,
            zIndex: 0,
            pointerEvents: 'none',
            left: `${format.dataBar.startPercent}%`,
            width: `${Math.max(0, format.dataBar.widthPercent)}%`,
            backgroundColor: format.dataBar.color,
            opacity: 0.55,
          }}
        />
      )}
      <span style={{ position: 'relative', zIndex: 1, display: 'inline-flex', alignItems: 'center', gap: 4 }}>
        {format.icon && <ConditionalIcon icon={format.icon.key} size={11} className="shrink-0" style={{ color: format.icon.color }} />}
        {children}
      </span>
    </td>
  );
}
