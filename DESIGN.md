# EmailTerminator — design

Desktop app, desktop-first and resizable, light and dark. Tone: trustworthy,
precise, efficient. Privacy is a table-stake feature, so the screens state it
plainly rather than selling it. The promise the screens make is auditability:
your mail stays on your machine, and every request the app sends is in source
code you can read and build yourself.

**Authority order.** The mockups in `docs/design/` are the visual source of
truth. This file states what each screen must contain and do. `PLAN.md` 2.4
says how the design becomes real components. Where the design system's own
`readme.md` disagrees with a mockup, the mockup wins — the two known
disagreements are named below.

**Additions without a mockup.** The 2026-09-26 revision of `PLAN.md` adds the
licence and evaluation reminders, opt-in cancel stats, database encryption, the Outlook.com
sign-in, and Maildir import. They go into the existing screens; no screen is
added. Each addition below is marked *(no mockup yet)*. Until a mockup exists,
this text governs it, and the new content follows the nearest mocked pattern
on the same screen.

Start from [`EmailTerminator Screens.dc.html`](docs/design/EmailTerminator%20Screens.dc.html),
the index of all 19 screens. [`Rail.dc.html`](docs/design/Rail.dc.html) is the
shared navigation, imported by 12 of them. Dark is
[`Dark A Espresso.dc.html`](docs/design/Dark%20A%20Espresso.dc.html).

---

## The system

The picked design system is
[`docs/design/_ds/organic-…/`](docs/design/_ds/organic-5588bfec-4a2c-478b-b965-0a1c6f9e0ae4/readme.md)
— warm and rounded: a cream-and-sand ground, a terracotta accent, a sage second
accent, over-rounded containers and pill buttons.

**Tokens.** `styles.css` holds the `:root` variables: three colour ramps
(neutral, accent, accent-2) at steps 100–900 generated in OKLCH on one shared
lightness scale, plus `--font-*`, `--space-*`, `--radius-*` and `--shadow-*`.
Never hard-code a hex, a font name, or a px value the tokens already carry.
Light steps (100–300) are tinted fills, hovers and subtle borders; 500 is a
role's base; dark steps (700–900) are text on tinted fills and pressed states.

**The alias layer is how screens actually consume the ramps.** Every mockup
defines a screen-level set — `--bg`, `--fg`, `--card`, `--line`, `--acc`,
`--sageSoft`, `--b1`…`--b4` — mapped onto ramp steps. Components reference
aliases, never ramps directly.

**Theme.** Dark redefines that alias layer and nothing else; the ramps do not
move. `Dark A Espresso.dc.html` is the definition to lift verbatim.

**Type.** Bricolage Grotesque for headings over Figtree for body.
**This contradicts the design system readme**, which names Caprasimo. Every
mockup overrides it to Bricolage Grotesque, so Bricolage Grotesque is correct.

**Icons.** Lucide, stroke width 2.75.

**Interaction states.** Themed, never browser defaults. Every interactive
element gets a hover tint and a pressed state one ramp step past its base.
Keyboard focus is `outline: 2px solid var(--color-accent); outline-offset: 2px`.
Disabled drops to 45% opacity.

**Direction.** Left-aligned and asymmetric. Flush-left headings, content hugging
the left edge with whitespace on the right. Round shapes with air around them —
they need the space to read as soft. No sharp corners, no hairline-only
geometry, no greys: the warmth is the point.

**Contrast limit.** The accent-to-ground pair is tuned to 3:1 — enough for
icons, large text and chrome, not for body copy. Paragraph-size text in the
accent uses `--color-accent-700` on the light ground.

### Two things in the bundle that are not usable as shipped

- `_ds_bundle.js` declares `"components":[]`. It is an empty shim.
- `_adherence.oxlintrc.json` targets React, names Caprasimo, and uses a rule
  oxlint lacks. Its intent — no raw hex, no raw px, no foreign font — is
  enforced by `ui/scripts/check-tokens.ts`. See `PLAN.md` 2.4.

The system's component classes (`.btn`, `.card`, `.table`) are not used by any
mockup and are not adopted. The tokens and the alias layer are what carry over.

---

## Screens

### S00 — Landing page (web)

[Mockup](docs/design/S00%20Landing.dc.html) · Not part of the app; it is the
site at emailterminator.com.

Content: product pitch, dashboard preview, privacy explanation framed as
"your mail stays local; read every request we send in the source", install
command, link to the GitHub repository. *(no mockup yet)* The price: free to
use, with reminders after 14 days until you buy a licence for
$29 or more ($19 or more in launch week).
Actions: copy the install command; open the repository; buy a licence through
the Polar checkout.

### S01 — Welcome / source picker

[Mockup](docs/design/S01%20Welcome.dc.html)

Content: product pitch, privacy statement. No accounts exist. The privacy
statement names every outbound request the app makes on its own, what each
sends, and where to switch it off: the update check; *(no mockup yet)* licence
activation and a check every 15 days, only on a licensed copy, sending the
key and a hash of the machine ID; and cancel stats, off unless the user
opts in. Each entry links to the source file that sends it. It also states
that the local data is encrypted.
Actions, IMAP first: connect a mailbox over IMAP with an app password;
*(no mockup yet)* sign in to Outlook.com; connect Gmail through a
bring-your-own OAuth client with guided setup; import an mbox file or
*(no mockup yet)* a Maildir folder.

*(no mockup yet)* The IMAP connect form that "Connect via IMAP" opens:
provider presets (Gmail, iCloud Mail, Fastmail, Yahoo Mail, other IMAP with
server and port), the address, the app password, a link to the provider's
app-password guide, and S16's sign-in failure card in place of the form when
the server refuses. Built at M1 from S01's cards and S16's error card.

### S02 — Scan progress

[Mockup](docs/design/S02%20Scan.dc.html)

Content: live counts — emails scanned, senders found, subscriptions detected,
newsletters detected.
Actions: cancel the scan; on completion, proceed to the dashboard.

### S03 — Dashboard (home)

[Mockup](docs/design/S03%20Dashboard.dc.html)

Content: estimated monthly spend, subscription count, newsletter count, emails
per year across all of it; top services by spend and by email volume. Where a
mailbox holds more than one currency, the headline figure names the dominant
one and lists the others beside it. There is no conversion.
Actions: unsubscribe all, showing emails/year removed; cancel all, showing
$/month saved; open a service's detail; go to the subscriptions or newsletters
list.

### S04 — Subscriptions list

[Mockup](docs/design/S04%20Subscriptions.dc.html)

Content per service: name, monthly cost, billing cadence, last charge, email
volume, price-increase flag, critical-service flag, status (active / canceling
/ canceled).
Actions: sort, filter, search; select several; unsubscribe or cancel the
selection; open a detail.

### S05 — Newsletters list

[Mockup](docs/design/S05%20Newsletters.dc.html)

Content per sender: name, frequency, total received, one-click-unsubscribe
availability, status.
Actions: sort, filter, search; select several; unsubscribe the selection; open
a detail.

### S06 — Service detail

[Mockup](docs/design/S06%20Service%20Detail.dc.html)

Content: spend history, email volume history, price-change history, receipt
list, critical-service warning where flagged. *(no mockup yet)* Where
`data/stats.json` has an entry: how many cancellations other users reported
and what share succeeded, by method.
Actions: unsubscribe; cancel via playbook; cancel via agent (experimental).

Carries both agent-unavailable states: extension absent, and extension present
with a protocol version out of step. Each names which side to update, and both
fall back to one-click unsubscribe and playbooks.

### S07 / S08 / S09 / S10 — Cancellation

[One-click result](docs/design/S07%20One-Click%20Result.dc.html) ·
[Playbook](docs/design/S08%20Playbook.dc.html) ·
[Agent](docs/design/S09%20Agent.dc.html) ·
[Critical confirm](docs/design/S10%20Critical%20Confirm.dc.html)

Three variants:

- **One-click** — content: the result, succeeded or failed.
- **Playbook** — content: curated step-by-step instructions with links;
  *(no mockup yet)* the playbook's reported success share where stats exist.
  Actions: mark a step done; open "improve this playbook".
- **Agentic (experimental)** — content: live agent progress; result, which is
  succeeded, needs-you, or failed. Actions: pause; take over control.

For a critical service, all three variants require an explicit confirmation
first. The agentic variant carries the same two unavailable states as S06.

### S11 — Bulk action review

[Mockup](docs/design/S11%20Bulk%20Review.dc.html)

Content: every service and sender affected, total $/month saved, total
emails/year removed, critical services excluded by default and listed as such;
then per-item progress and outcome as the run executes.
Actions: exclude or include individual items; confirm and execute.

### S12 — Sources settings

[Mockup](docs/design/S12%20Sources.dc.html)

Content per source: type (IMAP / Outlook / Gmail API / mbox / Maildir), last
sync, message count.
**Nothing syncs while the app is closed**, so "last sync" only advances while
the window is open and the screen must not imply otherwise.
Actions: add a source, the same three options as S01; re-import or re-sync;
disconnect.

### S13 — Intelligence settings

[Mockup](docs/design/S13%20Intelligence.dc.html)

Content: three tiers — Tier 0 no model, the default, where every core feature
works; Tier 1 local model; Tier 2 own API key (OpenAI-compatible,
Anthropic-compatible, OpenRouter, DeepSeek, custom endpoint) — with the current
tier and its connection status.
Actions: select a tier; pick or download a local model; enter an endpoint and
key; test the connection.

### S14 — Activity log

[Mockup](docs/design/S14%20Activity.dc.html)

Content: every action the app took — what, when, outcome — each with a link to
its evidence, either the email it came from or the agent session log. Evidence
can stop resolving when an imported mbox moves or an IMAP message is expunged;
that is a designed state, and the row keeps its subject, sender and date.
*(no mockup yet)* The evidence view: when, outcome, the request the app sent,
the email's subject, sender and date, and on request the original re-read from
the mailbox. Built at M3 as a dialog in the style of S07.
Actions: open evidence; filter by action type or outcome.

### S15 — Report an issue

[Mockup](docs/design/S15%20Report%20Issue.dc.html)

Content: a problem description field, and an exact preview of the prefilled
GitHub issue text — app version, OS, redacted error, never message content.
Actions: edit the description; open the prefilled issue on GitHub.

### S17 — General settings

[Mockup](docs/design/S17%20General.dc.html)

Content: appearance; sweep behaviour — confirm before bulk actions, with the
size from which a sweep is reviewed anyway, and leave critical services out
(the mockup's other two switches are not built; `PLAN.md` M3 says why);
local data location, and *(no mockup
yet)* whether it is encrypted with a key in the OS keychain or in a key file
beside it, in plain words; erase-all-data, which keeps the licence;
*(no mockup yet)* licence — evaluation days left, unregistered, or licensed;
the date of the last check; this device's place among the 3 allowed;
*(no mockup yet)* cancel stats — the opt-in switch and the exact payload one
report sends; *(no mockup yet)* support — a GitHub Sponsors link, shown to every
user whether licensed or not;
updates — current version, update state, and what a check sends (version, OS,
architecture, and the IP any HTTPS request carries; no identifier we invent);
browser integration — whether the native-messaging files are present on disk.
Actions: change appearance; configure sweep behaviour; change or reveal the
data location; erase all local data, behind a confirmation; set update
behaviour to Automatic (default), Notify only, or Off; check for updates now;
remove the browser integration files; *(no mockup yet)* enter a licence key,
open the Polar checkout, deactivate this device, switch cancel stats on or off, open GitHub Sponsors in
the browser.

### S18 — Command palette

[Mockup](docs/design/S18%20Command%20Palette.dc.html)

Invoked from anywhere. Content: search across services, senders, screens and
actions, with results as you type.
Actions: jump to any screen; run an action on a service or sender.

### S16 — Shared states

[Mockup](docs/design/S16%20Shared%20States.dc.html)

- Empty states: fresh install, and nothing-found on S03, S04, S05 and S14.
- Error states: mbox parse failure, IMAP auth failure, agent blocked.
- Experimental marker on agentic features.
- Critical-service warning, everywhere a flagged service appears.
- Agent unavailable: extension absent, or extension protocol out of step
  (S06, S07).
- Update ready: what changed, shown at the launch after an update installs.
- *(no mockup yet)* Evaluation reminder: a dialog with buy, enter a key, and
  continue evaluating, all enabled at once. It opens at launch, after a bulk
  run, and after every tenth single action, never during a scan, a
  critical-service confirmation or an agent run. Copy stays plain; no guilt.
- *(no mockup yet)* "Unregistered" marker in the rail on an unlicensed copy
  after the evaluation.
- *(no mockup yet)* Licence invalid: the key was refused, with Polar's reason.
- *(no mockup yet)* Device limit reached: the three devices on the licence,
  with a link to free one in Polar's customer portal.
- *(no mockup yet)* Licence check overdue: shown from day 15 of a token while
  offline, with the date the reminders return.
- *(no mockup yet)* Database key missing: the keychain entry is gone, so the
  local data cannot open; offer a fresh scan and state that the mailbox is
  untouched.
