export interface Point { x: number; y: number }
export interface Size { width: number; height: number }

/**
 * Posiciona o popover perto do ponto do mouse sem deixá-lo sair do viewport.
 * Tenta abrir para baixo/direita do ponto (como o tooltip padrão do Power
 * BI); se não houver espaço, inverte o eixo que faltar e, no limite, trava
 * na margem da tela.
 */
export function computePopoverPosition(anchor: Point, size: Size, viewport: Size, margin = 8, offset = 14): Point {
  let left = anchor.x + offset;
  let top = anchor.y + offset;

  if (left + size.width + margin > viewport.width) {
    left = anchor.x - size.width - offset;
  }
  if (top + size.height + margin > viewport.height) {
    top = anchor.y - size.height - offset;
  }

  left = Math.min(Math.max(left, margin), Math.max(margin, viewport.width - size.width - margin));
  top = Math.min(Math.max(top, margin), Math.max(margin, viewport.height - size.height - margin));

  return { x: left, y: top };
}
