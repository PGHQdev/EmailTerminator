# Design system bundle to SvelteKit components

Type: prototype
Status: open
Blocked by: —
ADR: docs/adr/NNNN-ui-composition.md (number assigned at resolution, in acceptance order)

## Question

How do the mockups and the picked design system become the real UI?

The inputs already exist: 18 `.dc.html` mockups in `docs/reference/design/`,
and the picked system at
`docs/reference/design/_ds/organic-5588bfec-4a2c-478b-b965-0a1c6f9e0ae4/`
holding `styles.css`, `_ds_bundle.js`, `_ds_manifest.json`, `readme.md`,
and an `_adherence.oxlintrc.json`.

Axes:

- Is `_ds_bundle.js` consumed as shipped, or are the tokens in `styles.css`
  re-expressed in the project's own components?
- The global default styling stack is Tailwind with Phosphor and BitsUI.
  Does that survive contact with this design system, or does it duplicate it?
- Light and dark are both designed. Which mechanism carries the theme?
- What state management the app needs, given the scan progress, bulk-action
  progress and activity log are all live-updating.
- Does `_adherence.oxlintrc.json` enter the lint setup?

Resolve by building a throwaway prototype of one screen — S03 Dashboard is
the densest — and judging the pipeline from it. Link the prototype from
this ticket. Then write the ADR.
