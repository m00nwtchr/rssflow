CREATE TABLE IF NOT EXISTS flows
(
    name        TEXT PRIMARY KEY    NOT NULL,
    content     TEXT                NOT NULL
);

CREATE TABLE IF NOT EXISTS websub
(
    uuid        BLOB PRIMARY KEY    NOT NULL,
    hub         TEXT                NOT NULL,
    topic       TEXT UNIQUE         NOT NULL,
    flow        TEXT UNIQUE         NOT NULL,
    secret      TEXT                NOT NULL,
    lease_end   INTEGER             ,
    subscribed  BOOLEAN DEFAULT 1   NOT NULL,
    FOREIGN KEY (flow)
        REFERENCES flows (name)
);