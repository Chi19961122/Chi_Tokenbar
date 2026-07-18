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
