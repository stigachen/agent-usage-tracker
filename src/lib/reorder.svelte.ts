// Pointer-based list reordering. HTML5 drag-and-drop is avoided because Tauri's
// file-drop handling swallows those events (notably on Windows).

/**
 * Tracks one drag over a vertical list. Items are elements marked with
 * `data-reorder-key`; `start` is wired to each item's pointerdown.
 */
export class Reorder {
  /** Key being dragged, or null when idle. */
  dragging = $state<string | null>(null);
  /** Current order while dragging; null when idle. */
  preview = $state<string[] | null>(null);

  constructor(private onDrop: (keys: string[]) => void) {}

  /** Returns `keys` in preview order while dragging, else unchanged. */
  order<T>(items: T[], key: (t: T) => string): T[] {
    if (!this.preview) return items;
    const p = this.preview;
    return [...items].sort((a, b) => p.indexOf(key(a)) - p.indexOf(key(b)));
  }

  /**
   * Wire to an item's pointerdown. The drag only begins once the pointer moves a few
   * pixels, so a plain click still works; the click that ends a drag is swallowed.
   */
  start(e: PointerEvent, key: string, keys: string[]) {
    if (e.button !== 0 || keys.length < 2) return;
    const list = (e.currentTarget as HTMLElement).closest("[data-reorder-list]");
    if (!list) return;
    const startY = e.clientY;
    let active = false;

    const move = (ev: PointerEvent) => {
      if (!active) {
        if (Math.abs(ev.clientY - startY) < 5) return;
        active = true;
        this.dragging = key;
        this.preview = [...keys];
      }
      ev.preventDefault();
      const items = [...list.querySelectorAll<HTMLElement>("[data-reorder-key]")];
      // Drop position = number of other items whose midpoint is above the pointer.
      const others = items.filter((el) => el.dataset.reorderKey !== key);
      const at = others.filter((el) => {
        const r = el.getBoundingClientRect();
        return ev.clientY > r.top + r.height / 2;
      }).length;
      const next = others.map((el) => el.dataset.reorderKey!);
      next.splice(at, 0, key);
      if (next.join("\n") !== this.preview!.join("\n")) this.preview = next;
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      if (!active) return;
      const swallow = (ev: MouseEvent) => {
        ev.stopPropagation();
        ev.preventDefault();
      };
      window.addEventListener("click", swallow, { capture: true, once: true });
      // If no click follows (pointer released elsewhere), drop the listener.
      setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
      const changed = this.preview!.join("\n") !== keys.join("\n");
      const result = this.preview!;
      this.dragging = null;
      this.preview = null;
      if (changed) this.onDrop(result);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  }
}
