//! Prepared schema-only palaces for ordinary unit tests, never application migrations.

#[path = "../../tests/common/db_fixture.rs"]
mod fixture;

use super::*;

pub(super) const PRODUCER_OUTPUT: &str = "MEMCASTLE_TEST_SCHEMA_OUTPUT";

impl SurrealStore {
    /// A private schema-only SurrealKV copy, or a fresh memory store outside nextest.
    pub(crate) async fn connect_for_tests() -> Self {
        let Some((cache, source)) = schema_template::default() else {
            return Self::connect_memory_for_tests().await;
        };
        let database =
            fixture::private_copy(&cache, &source).expect("copy schema-only database fixture");
        let store = Self::open_schema_fixture(database, StoreSync::Never).await;
        // Keep the ordinary unchanged-schema check rather than introducing a
        // test-only store lifecycle that can hide an invalid prepared schema.
        store.sync_schema().await.expect("sync copied schema");
        store
    }

    pub(super) async fn open_schema_fixture(path: PathBuf, sync: StoreSync) -> Self {
        let store = Self::connect(&Backend::Embedded { path, sync })
            .await
            .expect("open schema fixture");
        // Preserve the unit helper's original namespace/database contract.
        store
            .db
            .use_ns("test")
            .use_db("test")
            .await
            .expect("select unit-test database");
        store
    }
}

#[rstest::fixture]
fn schema_template(
    #[from(fixture::run_cache)] cache: Option<fixture::RunCache>,
) -> Option<(fixture::RunCache, PathBuf)> {
    let cache = cache?;
    let executable = std::env::current_exe().expect("resolve the schema producer test executable");
    let source = fixture::prepare(&cache, "schema-only", &executable, |database| {
        let directory = database.parent().unwrap();
        // Reuse the existing fresh-schema test rather than add another
        // executable target or re-enter Cargo from a running test suite.
        let mut command = std::process::Command::new(&executable);
        fixture::isolate(&mut command, directory);
        command
            .args([
                "--exact",
                "store::tests::migrations_apply_cleanly_and_are_idempotent",
                "--nocapture",
            ])
            .env(PRODUCER_OUTPUT, database);
        fixture::finish(&mut command, directory)
    })
    .expect("prepare the run's schema-only fixture");
    Some((cache, source))
}

pub(super) async fn finish_producer() {
    // The selected producer test owns this runtime. After its last store drops,
    // only datastore teardown tasks remain; let their flush finish before the
    // parent publishes a closed template (3.x has no awaited SDK close).
    let runtime = tokio::runtime::Handle::current();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while runtime.metrics().num_alive_tasks() != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("schema fixture datastore did not finish shutting down");
}
