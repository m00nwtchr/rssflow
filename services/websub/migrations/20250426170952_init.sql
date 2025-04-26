CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE IF NOT EXISTS subscription
(
    uuid       UUID PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
    topic      TEXT UNIQUE                                NOT NULL,
    hub        TEXT                                       NOT NULL,
    secret     TEXT                                       NOT NULL,
    lease_end  TIMESTAMPTZ,
    subscribed BOOLEAN          DEFAULT TRUE              NOT NULL
);
