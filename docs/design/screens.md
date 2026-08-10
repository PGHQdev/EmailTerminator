# EmailTerminator — v0 Screens

Desktop app. Design for desktop-first, resizable

window, light and dark. Tone: trustworthy, precise and efficient. Privacy is a table-stake

feature.

Reference mockups for every screen are mirrored in
[`docs/reference/design/`](../reference/design/). Use them as the visual
source of truth while building. Start from
[`EmailTerminator Screens.dc.html`](../reference/design/EmailTerminator%20Screens.dc.html)
(the index of all screens, light and dark). The picked design system lives in
[`_ds/organic-…/`](../reference/design/_ds/organic-5588bfec-4a2c-478b-b965-0a1c6f9e0ae4/readme.md)
(tokens in `styles.css`, usage rules in `readme.md`). Shared navigation:
[`Rail.dc.html`](../reference/design/Rail.dc.html). Dark theme:
[`Dark A Espresso.dc.html`](../reference/design/Dark%20A%20Espresso.dc.html).

## 0. Landing page (web)

Mockup: [`S00 Landing.dc.html`](../reference/design/S00%20Landing.dc.html)

Content: product pitch, dashboard preview, privacy explanation,
install command, link to the GitHub repository.
Actions:

- Copy the install command
- Open the GitHub repository

## 1. Welcome / Source picker

Mockup: [`S01 Welcome.dc.html`](../reference/design/S01%20Welcome.dc.html)

Content: product pitch, privacy statement. No accounts exist.
Actions:

- Import an mbox file
- Connect a mailbox via IMAP (app password)
- Connect Gmail via bring-your-own OAuth client (guided setup)

## 2. Scan progress

Mockup: [`S02 Scan.dc.html`](../reference/design/S02%20Scan.dc.html)

Content: live counts — emails scanned, senders found, subscriptions
detected, newsletters detected.
Actions:

- Cancel the scan
- (On completion) proceed to Dashboard

## 3. Dashboard (home)

Mockup: [`S03 Dashboard.dc.html`](../reference/design/S03%20Dashboard.dc.html)

Content: estimated monthly spend, subscription count, newsletter count,
emails per year across all of it; top services by spend and by email volume.
Actions:

- Unsubscribe all (shows emails/year removed)
- Cancel all (shows $/month saved)
- Open a service's detail
- Go to Subscriptions list / Newsletters list

## 3b. Command palette

Mockup: [`S18 Command Palette.dc.html`](../reference/design/S18%20Command%20Palette.dc.html)

Invoked from anywhere in the app.
Content: search across services, senders, screens, and actions;
results as you type.
Actions:

- Jump to any screen
- Run an action on a service/sender (unsubscribe, cancel, open detail)

## 4. Subscriptions list

Mockup: [`S04 Subscriptions.dc.html`](../reference/design/S04%20Subscriptions.dc.html)

Content per service: name, monthly cost, billing cadence, last charge,
email volume, price-increase flag, critical-service flag,
status (active / canceling / canceled).
Actions:

- Sort, filter, search
- Select multiple services
- Unsubscribe / cancel selected
- Open a service's detail

## 5. Newsletters list

Mockup: [`S05 Newsletters.dc.html`](../reference/design/S05%20Newsletters.dc.html)

Content per sender: name, frequency, total received,
one-click-unsubscribe availability, status.
Actions:

- Sort, filter, search
- Select multiple senders
- Unsubscribe selected
- Open a sender's detail

## 6. Service detail

Mockup: [`S06 Service Detail.dc.html`](../reference/design/S06%20Service%20Detail.dc.html)

Content: spend history, email volume history, price-change history,
receipt list, critical-service warning where flagged.
Actions:

- Unsubscribe
- Cancel via playbook
- Cancel via agent (experimental)

## 7. Cancellation flow

Mockups: [`S07 One-Click Result.dc.html`](../reference/design/S07%20One-Click%20Result.dc.html),
[`S08 Playbook.dc.html`](../reference/design/S08%20Playbook.dc.html),
[`S09 Agent.dc.html`](../reference/design/S09%20Agent.dc.html),
[`S10 Critical Confirm.dc.html`](../reference/design/S10%20Critical%20Confirm.dc.html)

Three variants:
a) One-click unsubscribe — content: result (succeeded / failed).
b) Playbook — content: curated step-by-step instructions with links.
   Actions: mark step done; open "improve this playbook" (community link).
c) Agentic (experimental) — content: live agent progress;
   result (succeeded / needs you / failed).
   Actions: pause; take over control.
For critical services, all three variants require an explicit confirmation
first.

## 8. Bulk action review

Mockup: [`S11 Bulk Review.dc.html`](../reference/design/S11%20Bulk%20Review.dc.html)

Content: every service/sender affected, total $/month saved, total
emails/year removed, critical services excluded by default (listed);
then per-item progress and success/failure as actions execute.
Actions:

- Exclude / include individual items
- Confirm and execute

## 9. Sources settings

Mockup: [`S12 Sources.dc.html`](../reference/design/S12%20Sources.dc.html)

Content per source: type (mbox / IMAP / Gmail API), last sync,
message count.
Actions:

- Add a source (same three options as screen 1)
- Re-import / re-sync
- Disconnect

## 10. Intelligence settings

Mockup: [`S13 Intelligence.dc.html`](../reference/design/S13%20Intelligence.dc.html)

Content: three tiers — Tier 0 no model (default; all core features work),
Tier 1 local model, Tier 2 own API key (OpenAI-compatible,
Anthropic-compatible, OpenRouter, DeepSeek, custom endpoint);
current tier and connection status.
Actions:

- Select tier
- Pick/download a local model
- Enter provider endpoint and key
- Test connection

## 10b. General settings

Mockup: [`S17 General.dc.html`](../reference/design/S17%20General.dc.html)

Content: appearance setting, sweep behavior settings, local data
location, erase-all-data option.
Actions:

- Change appearance
- Configure sweep behavior
- Change / reveal the data location
- Erase all local data (requires confirmation)

## 11. Activity log

Mockup: [`S14 Activity.dc.html`](../reference/design/S14%20Activity.dc.html)

Content: every action the app took — what, when, outcome — with a link to
evidence (the email or the agent session log).
Actions:

- Open evidence
- Filter by action type / outcome

## 12. Report an issue

Mockup: [`S15 Report Issue.dc.html`](../reference/design/S15%20Report%20Issue.dc.html)

Content: problem description field; exact preview of the prefilled GitHub
issue text (app version, OS, redacted error — never message content).
Actions:

- Edit description
- Open the prefilled issue on GitHub

## Shared states

Mockup: [`S16 Shared States.dc.html`](../reference/design/S16%20Shared%20States.dc.html)

- Empty states: fresh install, nothing found (screens 3, 4, 5, 11).
- Error states: mbox parse failure, IMAP auth failure, agent blocked.
- Experimental marker on agentic features.
- Critical-service warning on flagged services everywhere they appear.
