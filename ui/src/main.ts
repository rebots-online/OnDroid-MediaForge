/**
 * OnDroid MediaForge — placeholder front end for the Tauri 2 Android shell.
 *
 * T15 ships this page and nothing else: it renders the frozen design system's
 * colour tokens so the shell can be seen to boot, load its bundle and paint the
 * right ground colour on a device. The real screens are the frozen Stitch
 * exports under `LIBS/UI/STITCH/screens/` and they are wired in T17 — no screen
 * is invented here, and no command is called from here.
 *
 * Every value below is copied verbatim from the front matter of
 * `LIBS/UI/STITCH/DESIGN.md`, which is the single source of the palette.
 */

interface Token {
  name: string;
  hex: string;
}

interface TokenGroup {
  title: string;
  note: string;
  tokens: Token[];
}

const groups: TokenGroup[] = [
  {
    title: "Ground and text",
    note:
      "The neutral ramp is pinned, not derived from the copper accent: graphite #121316 " +
      "stepping up through five container tones, with warm off-white text.",
    tokens: [
      { name: "surface", hex: "#121316" },
      { name: "surface-dim", hex: "#0d0e10" },
      { name: "surface-bright", hex: "#33353a" },
      { name: "surface-container-lowest", hex: "#08090a" },
      { name: "surface-container-low", hex: "#181a1e" },
      { name: "surface-container", hex: "#1d1f23" },
      { name: "surface-container-high", hex: "#24262b" },
      { name: "surface-container-highest", hex: "#2c2e34" },
      { name: "surface-variant", hex: "#2c2e34" },
      { name: "background", hex: "#121316" },
      { name: "on-surface", hex: "#e8e6e3" },
      { name: "on-surface-variant", hex: "#b3afa9" },
      { name: "on-background", hex: "#e8e6e3" },
      { name: "inverse-surface", hex: "#e8e6e3" },
      { name: "inverse-on-surface", hex: "#2f3135" },
      { name: "outline", hex: "#8a857e" },
      { name: "outline-variant", hex: "#3f4045" },
      { name: "plate-highlight", hex: "#43444a" },
      { name: "plate-shadow", hex: "#070809" },
    ],
  },
  {
    title: "Accent",
    note:
      "Molten copper is the single primary action per view and the heat channel; " +
      "slate teal carries data flow along graph edges.",
    tokens: [
      { name: "primary", hex: "#ffb077" },
      { name: "on-primary", hex: "#4a1f00" },
      { name: "primary-container", hex: "#ed7014" },
      { name: "on-primary-container", hex: "#2e1200" },
      { name: "inverse-primary", hex: "#9a4a00" },
      { name: "surface-tint", hex: "#ffb077" },
      { name: "secondary", hex: "#7fd6cb" },
      { name: "on-secondary", hex: "#00352f" },
      { name: "secondary-container", hex: "#005049" },
      { name: "on-secondary-container", hex: "#c3f2ea" },
      { name: "tertiary", hex: "#a8c7e8" },
      { name: "on-tertiary", hex: "#0d2438" },
      { name: "tertiary-container", hex: "#2a4c6b" },
      { name: "on-tertiary-container", hex: "#d6e6f7" },
      { name: "error", hex: "#ff8a80" },
      { name: "on-error", hex: "#4a0e08" },
      { name: "error-container", hex: "#8c2118" },
      { name: "on-error-container", hex: "#ffd6d1" },
    ],
  },
  {
    title: "Heat / thermal state",
    note:
      "Drives the persistent thermal chip: cool, running nominally, sustained load, " +
      "governor derating, paused for cooling.",
    tokens: [
      { name: "heat-idle", hex: "#5a5b60" },
      { name: "heat-warm", hex: "#f07a1e" },
      { name: "heat-hot", hex: "#ff5722" },
      { name: "heat-throttle", hex: "#ffc033" },
      { name: "heat-paused", hex: "#7d7973" },
    ],
  },
  {
    title: "Port types",
    note:
      "Six typed ports. Colour is never the only carrier — each type also has a " +
      "mandatory distinct geometry, so type survives colour blindness and small size.",
    tokens: [
      { name: "port-audio", hex: "#ffa04d" },
      { name: "port-video", hex: "#5cc9bc" },
      { name: "port-image", hex: "#7fc95f" },
      { name: "port-mask", hex: "#e88ac4" },
      { name: "port-text", hex: "#93b8de" },
      { name: "port-tensor", hex: "#c9b96a" },
    ],
  },
  {
    title: "Node states",
    note:
      "The gating vocabulary. state-limited is a muted grey and carries no lock, no " +
      "price and no credit cost: capability outranks commerce, and a node the silicon " +
      "cannot run must never look like something to buy.",
    tokens: [
      { name: "state-ready", hex: "#9ad67f" },
      { name: "state-experimental", hex: "#ffb300" },
      { name: "state-limited", hex: "#6a6a6d" },
      { name: "state-metered", hex: "#d4c77f" },
      { name: "state-pro", hex: "#ffbb8a" },
    ],
  },
];

function swatch(token: Token): HTMLElement {
  const card = document.createElement("div");
  card.className = "swatch";

  const chip = document.createElement("div");
  chip.className = "chip";
  chip.style.background = token.hex;
  card.append(chip);

  const meta = document.createElement("div");
  meta.className = "meta";

  const name = document.createElement("span");
  name.className = "name";
  name.textContent = token.name;

  const hex = document.createElement("span");
  hex.className = "hex";
  hex.textContent = token.hex;

  meta.append(name, hex);
  card.append(meta);
  return card;
}

function section(group: TokenGroup): DocumentFragment {
  const fragment = document.createDocumentFragment();

  const heading = document.createElement("h2");
  heading.textContent = `${group.title} — ${group.tokens.length} tokens`;

  const note = document.createElement("p");
  note.className = "lede";
  note.textContent = group.note;

  const grid = document.createElement("div");
  grid.className = "swatches";
  for (const token of group.tokens) {
    grid.append(swatch(token));
  }

  fragment.append(heading, note, grid);
  return fragment;
}

function render(root: HTMLElement): void {
  const title = document.createElement("h1");
  title.textContent = "OnDroid MediaForge";

  const lede = document.createElement("p");
  lede.className = "lede";
  lede.textContent =
    "Shell placeholder. This page renders the frozen design system's colour tokens " +
    "and nothing else; the frozen screens are wired in T17.";

  root.append(title, lede);
  for (const group of groups) {
    root.append(section(group));
  }

  const total = groups.reduce((sum, group) => sum + group.tokens.length, 0);
  const footer = document.createElement("footer");
  footer.append(
    document.createTextNode(`${total} tokens from `),
    Object.assign(document.createElement("code"), {
      textContent: "LIBS/UI/STITCH/DESIGN.md",
    }),
    document.createTextNode(
      ". Media never leaves the device; the network is used only for model " +
        "downloads, entitlement sync and opt-in telemetry.",
    ),
  );
  root.append(footer);
}

const root = document.getElementById("app");
if (root) {
  render(root);
}
