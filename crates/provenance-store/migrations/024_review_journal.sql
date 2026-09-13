CREATE TABLE review_journal (
    scope_id TEXT NOT NULL,
    requirement_id TEXT NOT NULL,
    id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    request_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (scope_id, id),
    UNIQUE (scope_id, request_id),
    UNIQUE (scope_id, requirement_id, sequence)
);
