/**
 * OnDroid MediaForge — front end for the Tauri 2 Android shell.
 *
 * T17 wires the 24 frozen Stitch screen exports as the application's screens.
 * The shell boots, loads the bundle and renders the home screen. Navigation
 * is driven by a hash-based router that swaps screen HTML into the #app
 * container. No screen is invented here — every screen is a vendored frozen
 * export from LIBS/UI/STITCH/screens/.
 */

import { SCREENS, ScreenId, loadScreen } from "./screens/index";
import { renderAvailability } from "./screens/availability";

/** The default screen on app open. */
const HOME: ScreenId = "b1-home";

/** Parse the location hash into a ScreenId, falling back to home. */
function currentScreen(): ScreenId {
  const hash = window.location.hash.replace(/^#/, "");
  if (hash && (SCREENS as string[]).includes(hash)) {
    return hash as ScreenId;
  }
  return HOME;
}

/** Navigate to a screen by updating the hash. */
export function navigate(id: ScreenId): void {
  window.location.hash = id;
}

/** Render the current screen into #app. */
async function render(): Promise<void> {
  const root = document.getElementById("app");
  if (!root) return;

  const id = currentScreen();
  try {
    await loadScreen(id, root);
  } catch (err) {
    root.innerHTML = `<div style="padding:20px;color:#ff8a80">Failed to load screen: ${err}</div>`;
  }
}

/** Wire hash-change navigation. */
window.addEventListener("hashchange", render);

/** Boot. */
render();
