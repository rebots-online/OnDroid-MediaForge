---
name: OnDroid MediaForge Foundry
colors:
  surface: '#121316'
  surface-dim: '#0d0e10'
  surface-bright: '#33353a'
  surface-container-lowest: '#08090a'
  surface-container-low: '#181a1e'
  surface-container: '#1d1f23'
  surface-container-high: '#24262b'
  surface-container-highest: '#2c2e34'
  on-surface: '#e8e6e3'
  on-surface-variant: '#b3afa9'
  inverse-surface: '#e8e6e3'
  inverse-on-surface: '#2f3135'
  outline: '#8a857e'
  outline-variant: '#3f4045'
  surface-tint: '#ffb077'
  primary: '#ffb077'
  on-primary: '#4a1f00'
  primary-container: '#ed7014'
  on-primary-container: '#2e1200'
  inverse-primary: '#9a4a00'
  secondary: '#7fd6cb'
  on-secondary: '#00352f'
  secondary-container: '#005049'
  on-secondary-container: '#c3f2ea'
  tertiary: '#a8c7e8'
  on-tertiary: '#0d2438'
  tertiary-container: '#2a4c6b'
  on-tertiary-container: '#d6e6f7'
  error: '#ff8a80'
  on-error: '#4a0e08'
  error-container: '#8c2118'
  on-error-container: '#ffd6d1'
  background: '#121316'
  on-background: '#e8e6e3'
  surface-variant: '#2c2e34'
  plate-highlight: '#43444a'
  plate-shadow: '#070809'
  heat-idle: '#5a5b60'
  heat-warm: '#f07a1e'
  heat-hot: '#ff5722'
  heat-throttle: '#ffc033'
  heat-paused: '#7d7973'
  port-audio: '#ffa04d'
  port-video: '#5cc9bc'
  port-image: '#7fc95f'
  port-mask: '#e88ac4'
  port-text: '#93b8de'
  port-tensor: '#c9b96a'
  state-ready: '#9ad67f'
  state-experimental: '#ffb300'
  state-limited: '#6a6a6d'
  state-metered: '#d4c77f'
  state-pro: '#ffbb8a'
typography:
  headline-lg:
    fontFamily: Chivo
    fontSize: 30px
    fontWeight: '700'
    lineHeight: 38px
    letterSpacing: -0.01em
  headline-md:
    fontFamily: Chivo
    fontSize: 22px
    fontWeight: '700'
    lineHeight: 28px
  headline-sm:
    fontFamily: Chivo
    fontSize: 18px
    fontWeight: '600'
    lineHeight: 24px
  body-lg:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: 24px
  body-md:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 20px
  body-sm:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: '400'
    lineHeight: 18px
  telemetry:
    fontFamily: JetBrains Mono
    fontSize: 13px
    fontWeight: '450'
    lineHeight: 20px
  telemetry-sm:
    fontFamily: JetBrains Mono
    fontSize: 11px
    fontWeight: '400'
    lineHeight: 16px
  label-caps:
    fontFamily: Inter
    fontSize: 11px
    fontWeight: '700'
    lineHeight: 16px
    letterSpacing: 0.06em
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  base: 8px
  gutter: 16px
  margin-mobile: 20px
  touch-min: 44px
  sheet-max-height: 72vh
  node-row-height: 72px
---

# OnDroid MediaForge — Foundry design system

## Brand & Style

MediaForge is an **industrial instrument for a personal foundry**. The user's own
silicon does the work; the interface is the machine housing around it. It is not a
consumer AI toy and not a neon developer console — it is a tool that reports what
the hardware is actually doing, in units, without flattery.

The personality:

- **Machined, not glassy.** Surfaces read as anodized metal plates: a 1px top
  highlight, a soft bottom shadow, hairline outlines. Depth comes from tonal
  layering and edge treatment, never from blur or translucency.
- **Heat is information.** The copper-amber accent is a telemetry channel, not a
  brand flourish. It intensifies while silicon is under load and cools when a job
  pauses or throttles. Colour temperature *is* the thermal readout.
- **Honest about limits.** The interface states what this specific device can and
  cannot do, in seconds and megabytes. It never hides a hardware limit behind an
  upsell, and it never promises a speed this silicon will not reach.
- **Dense on demand.** Calm at rest, high-density when inspecting. The primary
  surface stays quiet; detail lives in bottom sheets the user pulls up.

### Anti-patterns — do not produce these

- No glassmorphism, frosted panels, or backdrop blur.
- No neon outer-glow borders or halo effects.
- No gradient-filled buttons; fills are flat, single-token.
- No mascots, celebratory motion, confetti, or congratulatory copy.
- No indeterminate shimmer or spinner where progress is knowable — segment counts
  are always known, so progress is always deterministic.
- No colour used as the sole carrier of meaning; always pair with glyph, shape, or
  text.
- No decorative stock photography or abstract 3D renders.

## Colors

Dark mode is the primary and only shipped appearance — this is a tool used on a
phone, often at night, over long jobs, on OLED panels where a graphite ground
costs almost no power.

The ground is **graphite** (`#121316`), stepping up through five container tones
to `#2c2e34`. Text is warm off-white (`#e8e6e3`) rather than pure white, which
keeps long transcript reading comfortable.

**Primary — molten copper.** `#ed7014` as the container fill, `#ffb077` as the
on-dark accent. Reserved for the single primary action per view and for the heat
channel. One primary action per screen, never two.

**Secondary — slate teal.** `#7fd6cb`. Carries data flow: graph edges, active
links, selected connections.

### Two constraints on this palette that are not aesthetic

Both were learned by observing what a design tool did to an earlier version of
this file, and both must hold on every future edit.

**Every custom token carries a hex no other token uses.** When this document was
first pushed to Stitch, nine of eighteen custom tokens vanished — and the pattern
was exact: every token whose value duplicated another token's value was
de-duplicated away, while every token with a unique value survived with its hex
intact. All five `state-*` tokens were lost as a group, because each happened to
share a colour with a port or a heat token. That is the worst possible loss here,
since the whole gating precedence rule depends on `state-limited` and `state-pro`
being visibly different things. Semantically distinct tokens therefore get
visually distinct values, even where the difference is a few percent of
lightness. This is also better design on its own terms: a green that means *ready*
and a green that means *image port* should never have been the same green.

**The neutral ramp is pinned, not derived.** A tool given only a copper primary
will generate a Material tonal palette seeded from it and produce a warm brown
neutral ramp — which it did, replacing graphite `#121316` with `#1c110b`
throughout. The graphite ground is a deliberate choice, not a by-product of the
accent, so the neutral must be supplied explicitly as an override rather than
inferred. When re-pushing this system, set the neutral override to `#121316`.

### Functional token groups

**Heat / thermal state** — drives the persistent thermal chip and the accent
temperature: `heat-idle` (device cool, nothing running) → `heat-warm` (job
running nominally) → `heat-hot` (sustained load) → `heat-throttle` (governor is
derating) → `heat-paused` (job paused for cooling, desaturated).

**Port types** — six typed ports, each with a hue **and** a mandatory distinct
geometry so type survives colour blindness and small size:

| Port type | Token | Geometry |
| --- | --- | --- |
| Audio | `port-audio` amber | circle |
| Video | `port-video` teal | square |
| Image | `port-image` green | triangle |
| Mask | `port-mask` magenta | diamond |
| Text | `port-text` steel blue | hexagon |
| Tensor | `port-tensor` brass | slot (rounded rectangle) |

Edges inherit the source port's hue and geometry marker. A type mismatch renders
the edge in `error` with a dashed stroke.

**Node states** — the gating vocabulary, used identically everywhere a node
appears: `state-ready` (green), `state-experimental` (amber), `state-limited`
(muted grey), `state-metered` (brass), `state-pro` (copper). These five tokens
plus the lock and heat glyphs express all seven node states defined in the
Components section.

## Typography

`Chivo` for headlines — an industrial grotesque with enough width and weight to
read as machine signage rather than app chrome. `Inter` for all body and UI text,
because it holds up at 13–14px on a dense phone panel. `JetBrains Mono` for every
machine-generated value without exception: milliseconds per frame, model
megabytes, tokens per second, credit balances, SoC identifiers, segment counters,
seeds.

That last rule is strict and it is the system's most useful signal — monospace
means *the machine measured this*, proportional means *a human wrote this*. A
user learns the distinction in one session and can then trust it.

Hierarchy comes from weight and scale, never from colour shifts. Most interaction
happens at `body-md` (14px). Headlines are reserved for screen titles and sheet
headers.

## Layout & Spacing

An 8px rhythm governs everything. `gutter` is 16px, mobile page margin is 20px.

**Touch is the primary input, so `touch-min` is 44px and it is a floor, not a
target.** Node ports, connection handles, and every control on the canvas meet it.
This is not optional polish: React Flow-class libraries are documented as
unusable on touch precisely because their hit targets assume a cursor.

**Three layout regimes:**

- **Narrow (Fold3 cover screen, ~24.5:9).** Single column, the linear recipe view
  only. No canvas. Controls stack full-width.
- **Standard phone.** Single column with bottom sheets for inspection. The recipe
  view is the default editor; the canvas is an explicit toggle.
- **Unfolded / tablet.** Two panes — canvas left, inspector docked right. The
  canvas is the product on this display; treat it as the primary layout rather
  than a stretched phone screen.

Bottom sheets cap at `sheet-max-height` (72vh) and are always dismissible by
drag. Node rows in the recipe view are `node-row-height` (72px) to fit a title,
a model line, and a backend chip without truncation.

## Elevation & Depth

Depth is **tonal layering plus edge treatment**, never shadow-heavy cards.

- Background sits at `surface`; panels step to `surface-container`; the active or
  selected element steps to `surface-container-high`.
- Every panel carries a 1px `outline-variant` hairline.
- The plate effect: a 1px `plate-highlight` inset top border and a soft
  `plate-shadow` beneath. This is what makes surfaces read as metal rather than
  paper or glass. Apply to node cards, sheets, and the thermal chip.
- Genuinely floating elements (bottom sheets, menus) get one soft shadow:
  `0 -8px 24px rgba(8,9,10,0.55)`. Nothing else casts a shadow.
- Focus state is a 2px `primary` outline at 2px offset.

## Shapes

Machined and rectilinear. `rounded-DEFAULT` (4px) on controls, chips, and inputs;
`rounded-lg` (8px) on panels, node cards, and sheets. Pills are used only for
status chips, never for primary buttons — a pill button reads consumer, and this
is an instrument.

## Components

### Thermal chip (persistent)

A small monospace chip in the top bar, always present. Shows the heat token
colour, a heat glyph, and the state word — `IDLE`, `RUNNING`, `SUSTAINED`,
`THROTTLING`, `COOLING`. When throttling or cooling it also shows the reason in
plain language. This chip is how the user learns the device is being protected
rather than the app being slow.

### Media-local chip (persistent)

A second top-bar chip stating that media stays on the device. Its copy is precise:
media never leaves this device. It must not claim the app is offline or that no
network is used — the app does download models and does sync entitlements. Getting
this copy wrong would be a false privacy claim.

### Node card

The core component, appearing in the recipe list, the canvas, and the palette.
Contains: node title (`headline-sm`), the model name (`body-sm`), a backend chip
(`telemetry-sm`: `NPU`, `GPU`, or `CPU`), a this-device time estimate
(`telemetry`), typed input and output ports with their geometry, and a state
indicator. Plate treatment. Selected state steps to
`surface-container-high` with a 2px `primary` left edge.

### The seven node states

Every place a node is shown, it is in exactly one of these. The state is carried
by glyph + text + token colour together.

1. **Ready** — `state-ready`. Backend chip and time estimate shown.
2. **Accelerated** — `state-ready` plus heat glyph and `NPU` chip.
3. **Needs model** — outlined, download glyph, size in `telemetry`, license line.
4. **Experimental** — `state-experimental`, flask glyph, honest slow estimate,
   requires explicit consent before first run.
5. **Metered** — `state-metered`, credit-cost chip, current balance, and a
   secondary "or unlock Pro" action.
6. **Pro locked** — `state-pro`, lock glyph, in-context upsell (never a
   full-screen interstitial from inside the editor).
7. **Tier limited** — `state-limited`, dimmed to 55% opacity, states the required
   hardware in plain words, and **always offers a substitute node that does run
   here**.

**Precedence rule, absolute:** tier-limited outranks every commercial state. A
node that is both unaffordable and unrunnable renders as tier-limited only. Never
show a lock, a price, or a credit cost for something this silicon cannot execute.
Tier-limited styling must be visually unmistakable from Pro-locked styling — grey
and matter-of-fact versus copper and promotional.

### Buttons

- **Primary** — flat `primary-container` fill, `on-primary` text, 4px radius. One
  per view.
- **Secondary** — 1px `outline` border, `primary` text, transparent fill.
- **Tertiary** — text only.
- **Destructive** — `error` text on transparent; filled only inside a confirmation
  sheet.

### Progress

Always deterministic. A segmented bar where each segment is a real processing
segment from the job plan, filling left to right. Beneath it, a monospace line
giving segment `n/N`, elapsed, and estimated remaining. When the governor derates,
the bar keeps its fill and the thermal chip changes — progress never goes
backwards and never resets.

### Inputs & validation

Top-aligned `label-caps` labels. 1px `outline-variant` border going 2px `primary`
on focus. Validation is inline, immediately below the field, in plain language and
without exclamation marks: "Needs a video input" rather than "Invalid".

### Bottom sheets

The inspection and decision surface. Drag handle, `headline-md` title, content,
actions pinned at the bottom above the safe area. Used for: node inspector, node
palette, model licenses, gating explanations, experimental consent, and purchase.

### Empty states

State what to do in one sentence and give one action. No illustrations, no
apologies.
