/**
 * Ícones de formatação condicional (v0.10, Frente B) — SVG inline pequenos
 * (sem emoji), para os sets 'arrows' | 'traffic' | 'flags'.
 */
import React from 'react';
import type { IconKey } from '../../../engine/conditionalFormat';

export function ConditionalIcon({
  icon,
  size = 12,
  className,
  style,
}: {
  icon: IconKey;
  size?: number;
  className?: string;
  style?: React.CSSProperties;
}) {
  const common = { width: size, height: size, viewBox: '0 0 16 16', className, style };
  switch (icon) {
    case 'arrow-up':
      return (
        <svg {...common} fill="currentColor" aria-label="seta para cima">
          <path d="M8 2 L14 12 L2 12 Z" />
        </svg>
      );
    case 'arrow-down':
      return (
        <svg {...common} fill="currentColor" aria-label="seta para baixo">
          <path d="M8 14 L2 4 L14 4 Z" />
        </svg>
      );
    case 'arrow-right':
      return (
        <svg {...common} fill="currentColor" aria-label="seta para o lado">
          <path d="M2 5 L2 11 L9 11 L9 14 L14 8 L9 2 L9 5 Z" />
        </svg>
      );
    case 'circle-green':
    case 'circle-yellow':
    case 'circle-red':
      return (
        <svg {...common} aria-label="indicador circular">
          <circle cx="8" cy="8" r="6" fill="currentColor" />
        </svg>
      );
    case 'flag-green':
    case 'flag-yellow':
    case 'flag-red':
      return (
        <svg {...common} aria-label="bandeira">
          <path d="M3 1 L3 15" stroke="currentColor" strokeWidth="1.3" fill="none" />
          <path d="M3 2 L13 4 L3 8 Z" fill="currentColor" />
        </svg>
      );
    default:
      return null;
  }
}

export const ICON_SET_LABELS: Record<'arrows' | 'traffic' | 'flags', string> = {
  arrows: 'Setas (baixo / lado / cima)',
  traffic: 'Semáforo (vermelho / amarelo / verde)',
  flags: 'Bandeiras (vermelha / amarela / verde)',
};

export const ICON_SET_PREVIEW: Record<'arrows' | 'traffic' | 'flags', IconKey[]> = {
  arrows: ['arrow-down', 'arrow-right', 'arrow-up'],
  traffic: ['circle-red', 'circle-yellow', 'circle-green'],
  flags: ['flag-red', 'flag-yellow', 'flag-green'],
};
