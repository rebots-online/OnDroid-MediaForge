// Subset the Material Symbols icon font to the glyphs the frozen screens use.
//
// Why this exists
// ---------------
// The pristine upstream font under vendored-in-code/ carries roughly 3,600
// icons and weighs 3.8 MB. The frozen complement references 67 of them. Shipping
// the whole face would put nearly four megabytes of unreachable glyphs in the
// APK for no benefit, so the shipped copy is derived by subsetting.
//
// The ligature set is read from the frozen screens rather than maintained by
// hand, because it changes whenever the complement is extended. Adding a screen
// that uses a new icon and forgetting to update a hard-coded list would ship a
// blank square where that icon should be — the failure would appear on device,
// not in a build log. Deriving it means that cannot happen.
//
// Material Symbols renders an icon when its NAME appears as text — the font maps
// the ligature "play_arrow" to a glyph. Subsetting therefore has to retain the
// characters of every name AND the ligature tables that combine them, which is
// why the text passed to the subsetter is the joined names and why the layout
// features are kept rather than stripped.
//
// Idempotent: same inputs produce the same output bytes, so re-running it
// changes nothing.

import { readFile, writeFile, readdir } from 'node:fs/promises'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import subsetFont from 'subset-font'

// This tool lives under ui/ rather than the repository's scripts/ because its
// dependency is a ui devDependency and Node resolves bare specifiers from the
// importing file's directory upward. scripts/vendor-stitch-assets.sh is the
// entry point and invokes it; nothing calls it directly.
const root = join(dirname(fileURLToPath(import.meta.url)), '../..')
const SCREENS = join(root, 'LIBS/UI/STITCH/screens')
const SOURCE = join(
  root,
  'vendored-in-code/registry.npmjs.org/material-symbols/material-symbols-outlined.woff2',
)
const OUTPUT = join(root, 'ui/src/assets/fonts/material-symbols-outlined-subset.woff2')

// The markup form is <span class="material-symbols-outlined ...">icon_name</span>.
const ICON_ELEMENT = /material-symbols-outlined[^>]*>([^<]+)</g

async function usedLigatures() {
  const names = new Set()
  for (const dir of await readdir(SCREENS)) {
    const html = await readFile(join(SCREENS, dir, 'screen.html'), 'utf8')
    for (const [, name] of html.matchAll(ICON_ELEMENT)) {
      const trimmed = name.trim()
      if (trimmed) names.add(trimmed)
    }
  }
  return [...names].sort()
}

const ligatures = await usedLigatures()
if (ligatures.length === 0) {
  console.error('FAILED: no icon ligatures found in the frozen screens')
  process.exit(1)
}

const source = await readFile(SOURCE)
const subset = await subsetFont(source, ligatures.join(' '), {
  targetFormat: 'woff2',
  // Ligature substitution is what turns "play_arrow" into a glyph. Dropping
  // these features would produce a font that renders the literal name.
  preserveNameIds: [],
})

await writeFile(OUTPUT, subset)

const pct = ((subset.length / source.length) * 100).toFixed(1)
console.log(`  ${ligatures.length} ligatures across the frozen complement`)
console.log(
  `  ${(source.length / 1024).toFixed(0)} KB -> ${(subset.length / 1024).toFixed(0)} KB (${pct}%)`,
)
console.log('subset icon font written')
