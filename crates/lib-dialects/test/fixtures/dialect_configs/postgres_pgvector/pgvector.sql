CREATE TABLE search (
    embedding VECTOR(1536)
);

CREATE TABLE IF NOT EXISTS msgs._all (
    bar VARCHAR NOT NULL,
    embd VECTOR(3072) DEFAULT NULL,
    baz VARCHAR
);

CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    half_embedding HALFVEC(512),
    sparse_embedding SPARSEVEC(1024),
    plain_vector VECTOR
);
