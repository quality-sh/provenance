ALTER TABLE review_journal RENAME TO requirement_journal;
CREATE TABLE review_journal (
    scope_id TEXT NOT NULL,
    requirement_id TEXT,
    id TEXT NOT NULL,
    sequence INTEGER,
    request_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    discussion_id TEXT,
    thread_id TEXT,
    message_id TEXT,
    parent_type TEXT,
    parent_id TEXT,
    version INTEGER,
    PRIMARY KEY (scope_id, id),
    UNIQUE (scope_id, request_id),
    UNIQUE (scope_id, requirement_id, sequence),
    UNIQUE (scope_id, discussion_id, version),
    UNIQUE (scope_id, message_id)
);
INSERT INTO review_journal (scope_id, requirement_id, id, sequence, request_id, payload)
    SELECT scope_id, requirement_id, id, sequence, request_id, payload FROM requirement_journal;
DROP TABLE requirement_journal;
CREATE INDEX discussion_parent ON review_journal(scope_id, parent_type, parent_id, discussion_id, version);
CREATE INDEX discussion_thread ON review_journal(scope_id, thread_id, message_id);
