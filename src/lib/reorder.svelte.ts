// Pointer-based list reordering. HTML5 drag-and-drop is avoided because Tauri's
// file-drop handling swallows those events (notably on Windows).

/** Distance from the scroll container's edge, in px, at which dragging auto-scrolls. */
const EDGE = 28;

function scrollParent(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const o = getComputedStyle(p).overflowY;
    if ((o === "auto" || o === "scroll") && p.scrollHeight > p.clientHeight) return p;
  }
  return null;
}

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
    const scroller = scrollParent(list as HTMLElement);
    let active = false;
    let y = startY;
    let frame = 0;

    // Drop position = number of other items whose midpoint is above the pointer.
    const place = () => {
      const items = [...list.querySelectorAll<HTMLElement>("[data-reorder-key]")];
      const others = items.filter((el) => el.dataset.reorderKey !== key);
      const at = others.filter((el) => {
        const r = el.getBoundingClientRect();
        return y > r.top + r.height / 2;
      }).length;
      const next = others.map((el) => el.dataset.reorderKey!);
      next.splice(at, 0, key);
      if (next.join("\n") !== this.preview!.join("\n")) this.preview = next;
    };
    // While the pointer is near the scroller's top/bottom edge, keep scrolling and re-place,
    // so targets outside the visible area can be reached.
    const autoScroll = () => {
      frame = 0;
      if (!scroller) return;
      const r = scroller.getBoundingClientRect();
      const dy = y < r.top + EDGE ? y - (r.top + EDGE) : y > r.bottom - EDGE ? y - (r.bottom - EDGE) : 0;
      if (!dy) return;
      const before = scroller.scrollTop;
      scroller.scrollTop += Math.sign(dy) * Math.min(Math.abs(dy) / 2 + 2, 14);
      if (scroller.scrollTop === before) return;
      place();
      frame = requestAnimationFrame(autoScroll);
    };

    const move = (ev: PointerEvent) => {
      y = ev.clientY;
      if (!active) {
        if (Math.abs(y - startY) < 5) return;
        active = true;
        this.dragging = key;
        this.preview = [...keys];
      }
      ev.preventDefault();
      place();
      if (!frame) frame = requestAnimationFrame(autoScroll);
    };
    const end = () => {
      cancelAnimationFrame(frame);
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
