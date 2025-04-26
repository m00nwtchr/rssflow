CREATE TABLE IF NOT EXISTS flows
(
    name    TEXT PRIMARY KEY NOT NULL,
    content JSONB            NOT NULL
);