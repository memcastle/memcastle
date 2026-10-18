-- A drawer may have one derived vector per immutable embedding space.
-- Legacy drawer.embedding has no provenance and is never read by profile-aware search.
DEFINE TABLE IF NOT EXISTS drawer_embedding SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS drawer ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS space ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS profile ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS provider ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS provider_type ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS connection ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS model ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS dimensions ON TABLE drawer_embedding TYPE int;
DEFINE FIELD IF NOT EXISTS pipeline_version ON TABLE drawer_embedding TYPE int;
DEFINE FIELD IF NOT EXISTS input_hash ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS content_hash ON TABLE drawer_embedding TYPE string;
DEFINE FIELD IF NOT EXISTS vector ON TABLE drawer_embedding TYPE array<float>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE drawer_embedding TYPE datetime;
DEFINE INDEX IF NOT EXISTS drawer_embedding_space_drawer ON TABLE drawer_embedding FIELDS space, drawer UNIQUE;
DEFINE INDEX IF NOT EXISTS drawer_embedding_drawer ON TABLE drawer_embedding FIELDS drawer;

-- Each indexed tier is an exclusive, permanent lease to one immutable space.
-- Other spaces of the same dimension use exact search; they NEVER enter this HNSW.
DEFINE TABLE IF NOT EXISTS embedding_slot SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS dimension ON TABLE embedding_slot TYPE int;
DEFINE FIELD IF NOT EXISTS space ON TABLE embedding_slot TYPE string;

DEFINE TABLE IF NOT EXISTS embedding_fast_768 SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS drawer ON TABLE embedding_fast_768 TYPE string;
DEFINE FIELD IF NOT EXISTS vector ON TABLE embedding_fast_768 TYPE array<float>;
DEFINE INDEX IF NOT EXISTS embedding_fast_768_drawer ON TABLE embedding_fast_768 FIELDS drawer UNIQUE;
DEFINE INDEX IF NOT EXISTS embedding_fast_768_hnsw ON TABLE embedding_fast_768 FIELDS vector HNSW DIMENSION 768 DIST COSINE TYPE F32;

DEFINE TABLE IF NOT EXISTS embedding_fast_1536 SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS drawer ON TABLE embedding_fast_1536 TYPE string;
DEFINE FIELD IF NOT EXISTS vector ON TABLE embedding_fast_1536 TYPE array<float>;
DEFINE INDEX IF NOT EXISTS embedding_fast_1536_drawer ON TABLE embedding_fast_1536 FIELDS drawer UNIQUE;
DEFINE INDEX IF NOT EXISTS embedding_fast_1536_hnsw ON TABLE embedding_fast_1536 FIELDS vector HNSW DIMENSION 1536 DIST COSINE TYPE F32;

DEFINE TABLE IF NOT EXISTS embedding_fast_3072 SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS drawer ON TABLE embedding_fast_3072 TYPE string;
DEFINE FIELD IF NOT EXISTS vector ON TABLE embedding_fast_3072 TYPE array<float>;
DEFINE INDEX IF NOT EXISTS embedding_fast_3072_drawer ON TABLE embedding_fast_3072 FIELDS drawer UNIQUE;
DEFINE INDEX IF NOT EXISTS embedding_fast_3072_hnsw ON TABLE embedding_fast_3072 FIELDS vector HNSW DIMENSION 3072 DIST COSINE TYPE F32;
