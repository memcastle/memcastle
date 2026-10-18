-- The index shipped before vectors recorded any model or profile.
DEFINE INDEX IF NOT EXISTS drawer_embedding_idx ON TABLE drawer
    FIELDS embedding HNSW DIMENSION 768 DIST COSINE TYPE F32;
