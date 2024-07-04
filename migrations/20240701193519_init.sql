CREATE TABLE IF NOT EXISTS flows
(
    uuid        BLOB PRIMARY KEY    NOT NULL,
    name        TEXT UNIQUE         NOT NULL,
    content     TEXT                NOT NULL
);