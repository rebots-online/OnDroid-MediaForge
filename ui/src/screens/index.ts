/**
 * Frozen screen registry — the 24 Stitch exports wired as the application's
 * screens. Coders do not re-implement or redesign any screen and do not invent
 * one that is not in the frozen complement.
 *
 * Each screen is an HTML file vendored from LIBS/UI/STITCH/screens/<name>/
 * by scripts/vendor-stitch-assets.sh. The HTML is loaded into the WebView
 * as-needed by the router in main.ts.
 */

/** Stable identity for a frozen screen. */
export type ScreenId =
  | "a1-welcome"
  | "a2-capability-result"
  | "a3-model-packs"
  | "a4-storage-grant"
  | "b1-home"
  | "b2-preset-gallery"
  | "b3-preset-detail"
  | "c1-recipe-view"
  | "c2-node-palette"
  | "c3-node-inspector"
  | "c4-validation-error"
  | "c5-canvas-unfolded"
  | "d1-node-state-legend"
  | "d2-paywall-device-aware"
  | "d3-tier-limited-sheet"
  | "d4-experimental-consent"
  | "d5-wallet-entitlement"
  | "e1-run-preflight"
  | "e2-job-monitor"
  | "e3-thermal-pause"
  | "e4-result-viewer"
  | "f1-model-manager"
  | "f2-model-license"
  | "f3-settings";

/** The frozen complement: every screen, in journey order. */
export const SCREENS: ScreenId[] = [
  "a1-welcome",
  "a2-capability-result",
  "a3-model-packs",
  "a4-storage-grant",
  "b1-home",
  "b2-preset-gallery",
  "b3-preset-detail",
  "c1-recipe-view",
  "c2-node-palette",
  "c3-node-inspector",
  "c4-validation-error",
  "c5-canvas-unfolded",
  "d1-node-state-legend",
  "d2-paywall-device-aware",
  "d3-tier-limited-sheet",
  "d4-experimental-consent",
  "d5-wallet-entitlement",
  "e1-run-preflight",
  "e2-job-monitor",
  "e3-thermal-pause",
  "e4-result-viewer",
  "f1-model-manager",
  "f2-model-license",
  "f3-settings",
];

/**
 * Load a frozen screen's HTML into the given container element.
 * The HTML is fetched from the local bundle (no outbound request).
 */
export async function loadScreen(
  id: ScreenId,
  container: HTMLElement,
): Promise<void> {
  const resp = await fetch(`/src/screens/${id}.html`);
  if (!resp.ok) {
    throw new Error(`failed to load screen ${id}: ${resp.status}`);
  }
  const html = await resp.text();
  container.innerHTML = html;
}
