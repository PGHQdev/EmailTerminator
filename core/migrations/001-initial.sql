-- The M1 schema (PLAN.md Part 4). Later milestones add tables in later files.

CREATE TABLE source (
    id              INTEGER PRIMARY KEY,
    kind            TEXT NOT NULL CHECK (kind IN ('imap', 'outlook', 'gmail', 'mbox', 'maildir')),
    label           TEXT NOT NULL,
    -- Per-kind settings as JSON: host, port, username for IMAP. Secrets live
    -- in the keychain (PLAN.md 2.2), never here.
    config          TEXT NOT NULL,
    last_sync_at    TEXT,
    message_count   INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL
);

-- One row per IMAP folder: the resume point for incremental sync.
CREATE TABLE mailbox (
    id              INTEGER PRIMARY KEY,
    source_id       INTEGER NOT NULL REFERENCES source (id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    uid_validity    INTEGER NOT NULL,
    highest_uid     INTEGER NOT NULL DEFAULT 0,
    UNIQUE (source_id, name)
);

-- A group of senders that bill or mail as one product.
CREATE TABLE service (
    id                  INTEGER PRIMARY KEY,
    name                TEXT NOT NULL,
    -- The registrable domain the senders share, or the merchant name for
    -- receipts relayed by a payment platform.
    group_key           TEXT NOT NULL UNIQUE,
    data_key            TEXT,
    is_critical         INTEGER NOT NULL DEFAULT 0,
    status              TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'canceling', 'canceled')),
    cadence             TEXT CHECK (cadence IN ('monthly', 'annual', 'irregular')),
    monthly_minor_units INTEGER,
    currency            TEXT
);

CREATE TABLE sender (
    id              INTEGER PRIMARY KEY,
    address         TEXT NOT NULL UNIQUE,
    display_name    TEXT,
    domain          TEXT NOT NULL,
    service_id      INTEGER REFERENCES service (id) ON DELETE SET NULL,
    first_seen      TEXT,
    last_seen       TEXT,
    message_count   INTEGER NOT NULL DEFAULT 0,
    classification  TEXT NOT NULL DEFAULT 'other'
                    CHECK (classification IN ('service', 'newsletter', 'other')),
    confidence      REAL NOT NULL DEFAULT 0,
    classified_by   TEXT NOT NULL DEFAULT 'parser'
                    CHECK (classified_by IN ('parser', 'playbook', 'model'))
);

-- Metadata only. No body is stored; the locator re-reads the original.
CREATE TABLE message (
    id                      INTEGER PRIMARY KEY,
    source_id               INTEGER NOT NULL REFERENCES source (id) ON DELETE CASCADE,
    mailbox_id              INTEGER REFERENCES mailbox (id) ON DELETE CASCADE,
    -- IMAP UID, mbox byte offset, or Maildir file name.
    locator                 TEXT NOT NULL,
    message_id              TEXT,
    sender_id               INTEGER REFERENCES sender (id),
    subject                 TEXT,
    date                    TEXT,
    list_unsubscribe        TEXT,
    list_unsubscribe_post   INTEGER NOT NULL DEFAULT 0,
    list_id                 TEXT,
    -- List or bulk mail that is not a receipt (Extraction::is_list).
    is_list                 INTEGER NOT NULL DEFAULT 0,
    -- DKIM d= domains, comma-separated; RFC 8058 eligibility reads them at M3.
    dkim_domains            TEXT NOT NULL DEFAULT '',
    UNIQUE (source_id, mailbox_id, locator)
);
CREATE INDEX message_sender ON message (sender_id);
CREATE INDEX message_message_id ON message (message_id);

-- An amount extracted from one message.
CREATE TABLE receipt (
    id                  INTEGER PRIMARY KEY,
    message_id          INTEGER NOT NULL UNIQUE REFERENCES message (id) ON DELETE CASCADE,
    kind                TEXT NOT NULL,
    amount_minor_units  INTEGER,
    currency            TEXT,
    charged_at          TEXT,
    invoice_ref         TEXT,
    merchant            TEXT,
    extracted_by        TEXT NOT NULL CHECK (extracted_by IN ('parser', 'playbook', 'model'))
);

-- A billing event after duplicates across folders are merged. S04's
-- price-increase flag and S06's price history read this table by date.
CREATE TABLE charge (
    id                  INTEGER PRIMARY KEY,
    service_id          INTEGER NOT NULL REFERENCES service (id) ON DELETE CASCADE,
    receipt_id          INTEGER NOT NULL REFERENCES receipt (id) ON DELETE CASCADE,
    amount_minor_units  INTEGER NOT NULL,
    currency            TEXT NOT NULL,
    charged_at          TEXT NOT NULL
);
CREATE INDEX charge_service ON charge (service_id, charged_at);

-- Monthly rollups, keyed by currency, rebuilt by every scan (PLAN.md 2.1).
CREATE TABLE aggregate (
    subject_kind        TEXT NOT NULL CHECK (subject_kind IN ('service', 'sender')),
    subject_id          INTEGER NOT NULL,
    month               TEXT NOT NULL,
    currency            TEXT NOT NULL DEFAULT '',
    spend_minor_units   INTEGER NOT NULL DEFAULT 0,
    message_count       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (subject_kind, subject_id, month, currency)
);

CREATE TABLE setting (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);

-- Subject and sender search inside a service detail. Contentless, because
-- the two columns come from two tables; a hit's rowid is the message id.
CREATE VIRTUAL TABLE message_fts USING fts5 (
    subject, sender_name, content = '', contentless_delete = 1
);

CREATE TRIGGER message_fts_insert AFTER INSERT ON message BEGIN
    INSERT INTO message_fts (rowid, subject, sender_name)
    VALUES (
        new.id,
        coalesce(new.subject, ''),
        coalesce((SELECT display_name FROM sender WHERE id = new.sender_id), '')
    );
END;

CREATE TRIGGER message_fts_delete AFTER DELETE ON message BEGIN
    DELETE FROM message_fts WHERE rowid = old.id;
END;
