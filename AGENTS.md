# EmailTerminator

## The four documents

The whole project is four files at the repository root plus one asset
directory. Nothing else is authoritative.

| File | Holds |
|---|---|
| `PLAN.md` | The build. Locked decisions and why, the resolved technical choices, repository layout, data model, milestones, release mechanics, corpus spec. |
| `CONTEXT.md` | The product. Origin, positioning, users, principles, v0 scope, deferred items with triggers, non-goals, monetization, success metrics. |
| `DESIGN.md` | The surface. The design system, then all 19 screens with their content and actions. |
| `AGENTS.md` | This file. How to work in the repository. |
| `docs/design/` | The mockups and the design-system bundle. Assets only, no prose. |

Read `PLAN.md` before writing code. Read `DESIGN.md` before writing UI. Read
`CONTEXT.md` when a question is about what the product is for.

## Process

There is no ADR directory, no ticket tracker, no decision map. Do not create
them. When a decision in `PLAN.md` turns out to be wrong, edit `PLAN.md` — the
sections numbered 1.1 to 1.6 hold the reasoning and each names what would
reopen it.

If your work contradicts a locked decision in Part 1, say so explicitly rather
than quietly working around it.

## Vocabulary

Use the terms these documents already use — service, sender, newsletter,
subscription, recipe, playbook, skill, tier, source. A concept that is not in
them is either language the project does not use, in which case reconsider it,
or a real gap, in which case add it to the document that owns it.

## Design work

`DESIGN.md` states what each screen must contain and do; the mockups in
`docs/design/` are the visual source of truth where the two disagree.
`Rail.dc.html` is the shared navigation that 12 screens import, and
`Dark A Espresso.dc.html` defines the dark theme.

The project builds its own token and theme files from the bundled design system
rather than consuming its component layer. `PLAN.md` 2.4 says what to take and
what to leave, including two places where the system's own readme is wrong.
