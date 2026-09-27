-- Outlook.com over Microsoft Graph (PLAN.md 1.6): a folder resumes from the
-- delta link of its last finished pass. IMAP folders leave it NULL.
ALTER TABLE mailbox ADD COLUMN delta_link TEXT;
