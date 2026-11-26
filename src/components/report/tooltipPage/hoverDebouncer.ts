/**
 * Debounce de ~150ms para abrir o popover de dica de ferramenta: agenda a
 * chamada, cancela a pendente se um novo ponto for sinalizado antes do
 * disparo (mouse passeando rápido pelo gráfico) e permite cancelar de todo
 * ao sair do ponto (mouseout).
 */
export class HoverDebouncer<T> {
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(private delayMs: number, private onFire: (payload: T) => void) {}

  schedule(payload: T): void {
    this.cancel();
    this.timer = setTimeout(() => {
      this.timer = null;
      this.onFire(payload);
    }, this.delayMs);
  }

  cancel(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  get isPending(): boolean {
    return this.timer !== null;
  }
}
