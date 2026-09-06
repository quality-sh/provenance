CREATE TABLE projection_validation (
    only_row INTEGER PRIMARY KEY CHECK (only_row = 1),
    version INTEGER NOT NULL
);
