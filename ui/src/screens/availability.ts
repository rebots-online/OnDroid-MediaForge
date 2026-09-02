/**
 * Availability presentation — maps each NodeAvailability variant to the row
 * presentation the d1-node-state-legend screen already renders.
 *
 * `NodeAvailability` derives plain `Serialize`, so it is externally tagged
 * and the JSON keys are the variant names verbatim: `Ready`, `Accelerated`,
 * `NeedsModel`, `Experimental`, `Metered`, `ProLocked`, `TierLimited`.
 *
 * TierLimited's presentation is taken from d1's seventh row — grey row-grey,
 * the label "Needs newer silicon", the substitute line — and not from
 * anywhere else. It must render with no lock, no price and no credit cost
 * anywhere in its markup.
 */

/** The seven NodeAvailability variant names, as they appear in JSON. */
export type AvailabilityVariant =
  | "Ready"
  | "Accelerated"
  | "NeedsModel"
  | "Experimental"
  | "Metered"
  | "ProLocked"
  | "TierLimited";

/** One row in the d1-node-state-legend presentation. */
export interface AvailabilityRow {
  /** CSS class for the row container (e.g. "row-green", "row-grey"). */
  rowClass: string;
  /** The badge label text. */
  label: string;
  /** Optional secondary line (e.g. substitute node name, credit cost). */
  detail?: string;
  /** Whether this variant shows a lock icon. */
  showsLock: boolean;
  /** Whether this variant shows a price or credit cost. */
  showsPrice: boolean;
}

/**
 * The presentation map. Each entry corresponds to a row in
 * d1-node-state-legend. TierLimited is the seventh row: grey, no lock, no
 * price, no credit cost.
 */
export const AVAILABILITY_PRESENTATION: Record<
  AvailabilityVariant,
  AvailabilityRow
> = {
  Ready: {
    rowClass: "row-green",
    label: "Ready",
    showsLock: false,
    showsPrice: false,
  },
  Accelerated: {
    rowClass: "row-blue",
    label: "Accelerated",
    showsLock: false,
    showsPrice: false,
  },
  NeedsModel: {
    rowClass: "row-amber",
    label: "Needs model download",
    showsLock: false,
    showsPrice: false,
  },
  Experimental: {
    rowClass: "row-purple",
    label: "Experimental",
    detail: "NPU delegate is experimental on this silicon",
    showsLock: false,
    showsPrice: false,
  },
  Metered: {
    rowClass: "row-copper",
    label: "Metered",
    detail: "Credits per run",
    showsLock: false,
    showsPrice: true,
  },
  ProLocked: {
    rowClass: "row-copper",
    label: "Pro",
    showsLock: true,
    showsPrice: false,
  },
  TierLimited: {
    rowClass: "row-grey",
    label: "Needs newer silicon",
    detail: "A substitute node is available at this tier",
    showsLock: false,
    showsPrice: false,
  },
};

/**
 * Render an availability state into the given container, using the d1
 * presentation. The variant name is extracted from the JSON tag.
 */
export function renderAvailability(
  state: { [key: string]: unknown },
  container: HTMLElement,
): void {
  // NodeAvailability is externally tagged serde JSON, so the variant name
  // is the first (and only) key.
  const variant = Object.keys(state)[0] as AvailabilityVariant;
  const row = AVAILABILITY_PRESENTATION[variant];
  if (!row) {
    container.innerHTML = `<div class="row-unknown">Unknown availability: ${variant}</div>`;
    return;
  }

  const detailHtml = row.detail
    ? `<span class="detail">${row.detail}</span>`
    : "";
  const lockHtml = row.showsLock
    ? `<span class="lock-icon material-symbols-outlined">lock</span>`
    : "";
  const priceHtml = row.showsPrice
    ? `<span class="price">${(state[variant] as { credits?: number })?.credits ?? ""}</span>`
    : "";

  container.innerHTML = `
    <div class="${row.rowClass}">
      <span class="label">${row.label}</span>
      ${detailHtml}
      ${lockHtml}
      ${priceHtml}
    </div>`;
}
