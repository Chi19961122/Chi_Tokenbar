function escapeHtml(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** Render a fixed settings choice with the shared analytics segmented vocabulary. */
export function segmentHtml(id: string, selected: string, options: ReadonlyArray<readonly [string, string]>): string {
  const buttons = options
    .map(
      ([value, label]) =>
        `<button type="button" data-val="${escapeHtml(value)}" class="${selected === value ? "on" : ""}">${escapeHtml(label)}</button>`,
    )
    .join("");
  return `<div class="seg seg-set" data-sid="${escapeHtml(id)}">${buttons}</div>`;
}

/** Read the selected value from one settings segmented control. */
export function readSegmentValue(root: ParentNode, id: string, fallback: string): string {
  return root.querySelector<HTMLElement>(`.seg-set[data-sid="${id}"] button.on[data-val]`)?.dataset.val || fallback;
}

/** Activate the clicked segment option. Returns false for unrelated clicks. */
export function activateSegment(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  const button = target.closest<HTMLButtonElement>(".seg-set button[data-val]");
  const segment = button?.closest<HTMLElement>(".seg-set");
  if (!button || !segment) return false;

  segment.querySelectorAll("button.on").forEach((item) => item.classList.remove("on"));
  button.classList.add("on");
  return true;
}

// ── multi-select variant (T-916) ─────────────────────────────────────────
//
// The same segmented vocabulary, but chips toggle *independently* (each keeps
// its own `.on` state) — for the sources multi-select. A `.seg-multi` never
// participates in `activateSegment`'s single-select radio behaviour.

/** Render a multi-select chip row. `selected` is the set of on values. */
export function segMultiHtml(
  id: string,
  selected: ReadonlyArray<string>,
  options: ReadonlyArray<readonly [string, string]>,
): string {
  const on = new Set(selected);
  const chips = options
    .map(
      ([value, label]) =>
        `<button type="button" data-val="${escapeHtml(value)}" class="${on.has(value) ? "on" : ""}">${escapeHtml(label)}</button>`,
    )
    .join("");
  return `<div class="seg seg-multi" data-sid="${escapeHtml(id)}">${chips}</div>`;
}

/** Read the on values from one multi-select chip row, in DOM order. */
export function readSegMultiValue(root: ParentNode, id: string): string[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>(`.seg-multi[data-sid="${id}"] button.on[data-val]`),
  )
    .map((b) => b.dataset.val)
    .filter((v): v is string => !!v);
}

/** Toggle the clicked chip's `.on` state. Returns false for unrelated clicks. */
export function toggleSegMulti(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  const button = target.closest<HTMLButtonElement>(".seg-multi button[data-val]");
  if (!button) return false;
  button.classList.toggle("on");
  return true;
}
