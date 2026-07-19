/** Stable analytics error codes from the Rust coordinator (snake_case). */
export type AnalyticsErrorCode = "superseded" | "cancelled" | "scan_failed";

export interface AnalyticsErrorInfo {
  code: AnalyticsErrorCode;
  message: string;
}

const PREFIX = "analytics_error:";

/** Decode a Tauri-surfaced error string into a structured code. */
export function parseAnalyticsError(raw: unknown): AnalyticsErrorInfo | null {
  const s =
    typeof raw === "string"
      ? raw
      : raw && typeof raw === "object" && "message" in raw
        ? String((raw as { message: unknown }).message)
        : raw != null
          ? String(raw)
          : "";
  if (!s.startsWith(PREFIX)) {
    // Also accept bare code payloads if the runtime ever serializes objects.
    if (s === "superseded" || s === "cancelled" || s === "scan_failed") {
      return { code: s, message: s };
    }
    return null;
  }
  const rest = s.slice(PREFIX.length);
  const colon = rest.indexOf(":");
  const code = colon >= 0 ? rest.slice(0, colon) : rest;
  const message = colon >= 0 ? rest.slice(colon + 1) : rest;
  if (code === "superseded" || code === "cancelled" || code === "scan_failed") {
    return { code, message };
  }
  return null;
}

export function isNoResultCode(code: AnalyticsErrorCode): boolean {
  return code === "superseded" || code === "cancelled";
}
