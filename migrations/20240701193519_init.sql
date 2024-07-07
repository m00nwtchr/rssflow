CREATE TABLE IF NOT EXISTS flows
(
    name        TEXT PRIMARY KEY    NOT NULL,
    content     TEXT                NOT NULL
);

CREATE TABLE IF NOT EXISTS websub
(
    uuid        BLOB PRIMARY KEY    NOT NULL,
    topic       TEXT UNIQUE         NOT NULL,
    hub         TEXT                NOT NULL,
    secret      TEXT                NOT NULL,
    lease_end   INTEGER             ,
    subscribed  BOOLEAN DEFAULT 1   NOT NULL
);

CREATE TABLE IF NOT EXISTS websub_flows
(
    topic       TEXT                NOT NULL,
    flow        TEXT                NOT NULL,

    PRIMARY KEY (topic, flow),
    FOREIGN KEY (topic) REFERENCES websub (topic),
    FOREIGN KEY (flow)  REFERENCES flows  (name) ON DELETE CASCADE
);