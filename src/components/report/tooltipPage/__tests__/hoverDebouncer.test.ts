import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { HoverDebouncer } from '../hoverDebouncer';

describe('HoverDebouncer', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('dispara o callback só depois do delay', () => {
    const onFire = vi.fn();
    const d = new HoverDebouncer<string>(150, onFire);
    d.schedule('ponto_a');
    vi.advanceTimersByTime(149);
    expect(onFire).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(onFire).toHaveBeenCalledWith('ponto_a');
  });

  it('reentrar em outro ponto antes do delay cancela o anterior (só o último dispara)', () => {
    const onFire = vi.fn();
    const d = new HoverDebouncer<string>(150, onFire);
    d.schedule('ponto_a');
    vi.advanceTimersByTime(80);
    d.schedule('ponto_b');
    vi.advanceTimersByTime(150);
    expect(onFire).toHaveBeenCalledTimes(1);
    expect(onFire).toHaveBeenCalledWith('ponto_b');
  });

  it('cancel() antes do delay nunca dispara (mouseout no meio do caminho)', () => {
    const onFire = vi.fn();
    const d = new HoverDebouncer<string>(150, onFire);
    d.schedule('ponto_a');
    vi.advanceTimersByTime(100);
    d.cancel();
    vi.advanceTimersByTime(200);
    expect(onFire).not.toHaveBeenCalled();
    expect(d.isPending).toBe(false);
  });

  it('isPending reflete corretamente o estado do timer', () => {
    const d = new HoverDebouncer<string>(150, () => {});
    expect(d.isPending).toBe(false);
    d.schedule('x');
    expect(d.isPending).toBe(true);
    vi.advanceTimersByTime(150);
    expect(d.isPending).toBe(false);
  });
});
