-- M3: the activity log (S14), unsubscribe state, and the RFC 8058 verdict
-- per message (PLAN.md Part 4).

ALTER TABLE message ADD COLUMN one_click INTEGER NOT NULL DEFAULT 0;

-- When the app last unsubscribed the sender. Null while subscribed.
ALTER TABLE sender ADD COLUMN unsubscribed_at TEXT;

CREATE TABLE action (
    id                  INTEGER PRIMARY KEY,
    kind                TEXT NOT NULL CHECK (kind IN ('unsubscribe', 'playbook', 'agent', 'sync')),
    -- The name the row shows. Text, so the row outlives what it names.
    target              TEXT NOT NULL,
    sender_id           INTEGER REFERENCES sender (id) ON DELETE SET NULL,
    service_id          INTEGER REFERENCES service (id) ON DELETE SET NULL,
    source_id           INTEGER REFERENCES source (id) ON DELETE SET NULL,
    at                  TEXT NOT NULL,
    outcome             TEXT NOT NULL CHECK (outcome IN ('succeeded', 'failed', 'needs_you')),
    -- One plain line: what the other side answered, or why nothing was sent.
    detail              TEXT NOT NULL,
    -- What the app sent, such as "POST https://…". Null when it sent nothing.
    request             TEXT,
    -- The email the action came from. The three copies keep its subject,
    -- sender and date when the message row goes (S14's designed state).
    message_id          INTEGER REFERENCES message (id) ON DELETE SET NULL,
    evidence_subject    TEXT,
    evidence_from       TEXT,
    evidence_date       TEXT
);
CREATE INDEX action_at ON action (at);

-- Messages stored before this migration have no one-click verdict. Fetch
-- each folder again from its oldest message that asks for a one-click POST;
-- the scan updates rows it already has.
UPDATE mailbox SET highest_uid = coalesce(
    (SELECT min(CAST(m.locator AS INTEGER)) - 1 FROM message m
     WHERE m.mailbox_id = mailbox.id AND m.list_unsubscribe_post = 1),
    highest_uid
);
