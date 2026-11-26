import { describe, expect, it } from 'vitest';
import { computePopoverPosition } from '../position';

describe('computePopoverPosition', () => {
  const viewport = { width: 1366, height: 768 };
  const size = { width: 320, height: 240 };

  it('abre para baixo/direita do ponto quando há espaço', () => {
    const pos = computePopoverPosition({ x: 100, y: 100 }, size, viewport);
    expect(pos.x).toBeGreaterThan(100);
    expect(pos.y).toBeGreaterThan(100);
    expect(pos.x + size.width).toBeLessThanOrEqual(viewport.width);
    expect(pos.y + size.height).toBeLessThanOrEqual(viewport.height);
  });

  it('perto da borda direita, inverte para a esquerda do ponto', () => {
    const pos = computePopoverPosition({ x: 1350, y: 100 }, size, viewport);
    expect(pos.x + size.width).toBeLessThanOrEqual(viewport.width - 8 + 1);
    expect(pos.x).toBeGreaterThanOrEqual(0);
  });

  it('perto da borda inferior, inverte para cima do ponto', () => {
    const pos = computePopoverPosition({ x: 100, y: 760 }, size, viewport);
    expect(pos.y + size.height).toBeLessThanOrEqual(viewport.height - 8 + 1);
    expect(pos.y).toBeGreaterThanOrEqual(0);
  });

  it('no canto inferior direito, nunca sai do viewport em nenhum eixo', () => {
    const pos = computePopoverPosition({ x: 1360, y: 765 }, size, viewport);
    expect(pos.x).toBeGreaterThanOrEqual(0);
    expect(pos.y).toBeGreaterThanOrEqual(0);
    expect(pos.x + size.width).toBeLessThanOrEqual(viewport.width);
    expect(pos.y + size.height).toBeLessThanOrEqual(viewport.height);
  });

  it('no canto superior esquerdo (0,0), trava na margem sem valores negativos', () => {
    const pos = computePopoverPosition({ x: 0, y: 0 }, size, viewport);
    expect(pos.x).toBeGreaterThanOrEqual(0);
    expect(pos.y).toBeGreaterThanOrEqual(0);
  });

  it('tela pequena (1366×768): popover de 320×240 sempre cabe', () => {
    for (const anchor of [{ x: 0, y: 0 }, { x: 1366, y: 768 }, { x: 683, y: 384 }]) {
      const pos = computePopoverPosition(anchor, size, viewport);
      expect(pos.x + size.width).toBeLessThanOrEqual(viewport.width);
      expect(pos.y + size.height).toBeLessThanOrEqual(viewport.height);
    }
  });
});
