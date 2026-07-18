import { resolveLocale, setLocale, t } from "./i18n";

export interface SharePreviewPayload {
  dataUrl: string;
  locale: string;
}

export interface SharePreviewDeps {
  getPreview: () => Promise<SharePreviewPayload>;
  closeWindow: () => Promise<void>;
}

const tauriDeps: SharePreviewDeps = {
  async getPreview() {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<SharePreviewPayload>("get_share_preview");
  },
  async closeWindow() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  },
};

export function isSharePreviewHash(hash: string): boolean {
  return hash === "#share-preview";
}

function renderPreview(payload: SharePreviewPayload): void {
  const frame = document.createElement("main");
  frame.className = "share-preview-scrim";

  const image = document.createElement("img");
  image.className = "share-preview-image";
  image.src = payload.dataUrl;
  image.alt = t("share.previewTitle");

  const hint = document.createElement("div");
  hint.className = "share-preview-hint";
  hint.textContent = t("share.previewHint");

  frame.append(image, hint);
  document.body.replaceChildren(frame);
}

/** Minimal boot for the dedicated Tauri preview window. It deliberately reads
 * only the in-memory PNG payload and never starts the main app subscriptions. */
export async function bootSharePreview(
  deps: SharePreviewDeps = tauriDeps,
): Promise<() => void> {
  document.body.className = "share-preview-body";
  document.body.replaceChildren();

  let closing = false;
  const closeOnce = async () => {
    if (closing) return;
    closing = true;
    await deps.closeWindow();
  };
  const onClick = () => void closeOnce();
  const onKeydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") void closeOnce();
  };
  window.addEventListener("click", onClick);
  window.addEventListener("keydown", onKeydown);

  try {
    const payload = await deps.getPreview();
    setLocale(resolveLocale(payload.locale));
    renderPreview(payload);
  } catch {
    void closeOnce();
  }

  return () => {
    window.removeEventListener("click", onClick);
    window.removeEventListener("keydown", onKeydown);
  };
}
