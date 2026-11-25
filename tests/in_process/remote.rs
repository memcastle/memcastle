//! Daemon behavior against the independently managed SurrealDB 4.0 service.

use crate::common::{TestDaemon, wait_for_job_status};
use memcastle::config::{Secret, StoreConfig};
use memcastle::domain::{Job, JobStatus};
use serde_json::json;

#[tokio::test]
#[ignore = "requires MEMCASTLE_TEST_SURREAL_WS; run mise run test:remote"]
async fn a_remote_palace_serves_jobs_and_retrieval_after_root_sign_in() {
    let url = std::env::var("MEMCASTLE_TEST_SURREAL_WS")
        .expect("set MEMCASTLE_TEST_SURREAL_WS to the disposable SurrealDB 4.0.1 service URL");
    // Each invocation owns a namespace; both daemon starts deliberately share it.
    let namespace = format!("remote_{}", uuid::Uuid::new_v4().simple());
    let configure = |config: &mut memcastle::config::Config| {
        config.store = StoreConfig::Remote {
            url: url.clone(),
            namespace: namespace.clone(),
            database: "palace".into(),
            username: "root".into(),
            password: Secret::new("disposable-surrealdb-test-password"),
        };
    };
    let daemon = TestDaemon::start_configured(configure).await;
    let client = reqwest::Client::new();
    let status: serde_json::Value = client
        .get(format!("{}/api/status", daemon.base_url))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["datastore"]["backend"], "remote");
    client.post(format!("{}/api/notes", daemon.base_url))
        .json(&json!({"wing":"remote", "room":"notes", "content":"remote zebra memory", "requested_by":"remote-test"}))
        .send().await.unwrap().error_for_status().unwrap();
    let job: Job = client
        .post(format!("{}/api/jobs", daemon.base_url))
        .json(&json!({"type":"audit", "requested_by":"remote-test"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    wait_for_job_status(&client, &daemon.base_url, job.id, JobStatus::Completed).await;
    let search: serde_json::Value = client
        .get(format!("{}/api/search?q=zebra", daemon.base_url))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search[0]["content"], "remote zebra memory");
    daemon.shutdown().await;

    // Reopening the same schema retains the first daemon's durable job.
    let reopened = TestDaemon::start_configured(configure).await;
    let stored: Job = client
        .get(format!("{}/api/jobs/{}", reopened.base_url, job.id))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stored.status, JobStatus::Completed);
    reopened.shutdown().await;
}
