CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE person (
    public_key TEXT PRIMARY KEY NOT NULL,
    name TEXT DEFAULT NULL,
    about TEXT DEFAULT NULL,
    picture TEXT DEFAULT NULL,
    nip05 TEXT DEFAULT NULL,
    following INTEGER DEFAULT 0
) WITHOUT ROWID;

CREATE TABLE person_relay (
    person TEXT NOT NULL,
    relay TEXT NOT NULL,
    recommended INTEGER DEFAULT 0,
    last_fetched TEXT DEFAULT NULL,
    UNIQUE(person, relay)
);

CREATE TABLE person_contact (
    person TEXT NOT NULL,
    contact TEXT NOT NULL,
    relay TEXT DEFAULT NULL,
    petname TEXT DEFAULT NULL,
    UNIQUE(person, contact)
);

CREATE TABLE event (
    id TEXT PRIMARY KEY NOT NULL,
    public key TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    kind INTEGER NOT NULL,
    content TEXT NOT NULL,
    ots TEXT DEFAULT NULL
) WITHOUT ROWID;

CREATE TABLE event_tag (
    event TEXT NOT NULL,
    label TEXT NOT NULL,
    field0 TEXT DEFAULT NULL,
    field1 TEXT DEFAULT NULL,
    field2 TEXT DEFAULT NULL,
    field3 TEXT DEFAULT NULL
);

CREATE TABLE relay (
    url TEXT PRIMARY KEY NOT NULL,
    last_up TEXT DEFAULT NULL,
    last_try TEXT DEFAULT NULL,
    last_fetched TEXT DEFAULT NULL
) WITHOUT ROWID;

INSERT INTO SETTINGS (key, value) values ('version', '1');
