import { resolveLocale, setLocale, t } from "./i18n";

export interface SharePreviewPayload {
  /** Absolute path to backend-owned PNG; preferred over dataUrl. */
  filePath?: string | null;
  /** Legacy / browser: may still carry a data URL (Tauri file-backed leaves null). */
  dataUrl: string | null;
  locale: string;
}

type Unlisten = () => void;

export interface SharePreviewDeps {
  getPreview: () => Promise<SharePreviewPayload>;
  listenForUpdates: (listener: () => void) => Promise<Unlisten>;
  closeWindow: () => Promise<void>;
}

const SHARE_PREVIEW_UPDATED_EVENT = "share-preview-updated";

const tauriDeps: SharePreviewDeps = {
  async getPreview() {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<SharePreviewPayload>("get_share_preview");
  },
  async listenForUpdates(listener) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen(SHARE_PREVIEW_UPDATED_EVENT, listener);
  },
  async closeWindow() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  },
};

export function isSharePreviewHash(hash: string): boolean {
  return hash === "#share-preview";
}

function renderGenerating(): void {
  const frame = document.createElement("main");
  frame.className = "share-preview-scrim";

  const hint = document.createElement("div");
  hint.className = "share-preview-hint";
  hint.textContent = t("share.previewGenerating");

  frame.appendChild(hint);
  document.body.replaceChildren(frame);
}

function renderPreview(dataUrl: string): void {
  const frame = document.createElement("main");
  frame.className = "share-preview-scrim";

  const image = document.createElement("img");
  image.className = "share-preview-image";
  image.src = dataUrl;
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
  renderGenerating();

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

  const applyPayload = async (payload: SharePreviewPayload) => {
    setLocale(resolveLocale(payload.locale));
    let src = payload.dataUrl;
    if (payload.filePath) {
      try {
        const { convertFileSrc } = await import("@tauri-apps/api/core");
        src = convertFileSrc(payload.filePath);
      } catch {
        src = payload.filePath;
      }
    }
    if (src) {
      renderPreview(src);
    } else {
      renderGenerating();
    }
  };
  let latestPull = 0;
  const pullPreview = async () => {
    const pull = ++latestPull;
    const payload = await deps.getPreview();
    if (pull === latestPull) {
      await applyPayload(payload);
    }
  };

  let unlisten: Unlisten | undefined;
  try {
    await pullPreview();
    unlisten = await deps.listenForUpdates(() => {
      void pullPreview().catch(() => undefined);
    });
    await pullPreview();
  } catch {
    void closeOnce();
  }

  return () => {
    unlisten?.();
    window.removeEventListener("click", onClick);
    window.removeEventListener("keydown", onKeydown);
  };
}
