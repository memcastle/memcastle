//! The credential lifecycle against a fake provider: signing in, using, renewing, losing and replacing a credential.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::fake::{self, Provider};
use super::oauth::Pkce;
use super::store::{CredentialStore, FileStore};
use super::*;

const SOURCE: &str = "demo";

struct Fixture {
    provider: Arc<Provider>,
    credentials: Credentials,
    /// A second view of the same files, to see what was kept.
    files: FileStore,
    _dir: tempfile::TempDir,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("credentials");
    Fixture {
        provider: fake::start().await,
        credentials: Credentials::with_timing(
            Box::new(FileStore::new(path.clone())),
            Timing {
                min_poll: Duration::from_millis(10),
                slow_down_step: Duration::from_millis(10),
            },
        ),
        files: FileStore::new(path),
        _dir: dir,
    }
}

impl Fixture {
    async fn authorize(&self, binding: OAuthBinding) -> OAuthBinding {
        let key = binding.configuration.clone();
        let challenge = self.credentials.begin_bound(binding.clone()).await.unwrap();
        if challenge.kind == FlowKind::Browser {
            let asked = authorization(challenge.url.as_deref().unwrap());
            assert_eq!(asked["client_id"], binding.client_id);
            assert_eq!(
                call_back(
                    &asked["redirect_uri"],
                    &[("code", "good-code"), ("state", &asked["state"])]
                )
                .await,
                200
            );
        }
        loop {
            match self
                .credentials
                .wait(&key, &challenge.flow, Duration::from_millis(100))
                .await
                .unwrap()
            {
                FlowStatus::SignedIn(_) => break,
                FlowStatus::Pending => {}
            }
        }
        self.credentials.binding(binding).unwrap()
    }
    /// The device flow, from the challenge to the end.
    async fn sign_in(&self) -> SignedIn {
        let requirement = self.provider.requirement();
        let challenge = self.credentials.begin(SOURCE, &requirement).await.unwrap();
        self.finish(&challenge.flow).await
    }

    async fn finish(&self, flow: &str) -> SignedIn {
        for _ in 0..200 {
            match self
                .credentials
                .wait(SOURCE, flow, Duration::from_millis(100))
                .await
                .unwrap()
            {
                FlowStatus::SignedIn(done) => return done,
                FlowStatus::Pending => {}
            }
        }
        panic!("the sign-in never finished");
    }

    /// An access token, asked for as the mining runtime asks: from a blocking context.
    async fn token(&self) -> Result<Secret> {
        self.token_for(self.provider.requirement()).await
    }

    async fn token_for(&self, requirement: OAuthRequirement) -> Result<Secret> {
        let credentials = self.credentials.clone();
        tokio::task::spawn_blocking(move || credentials.access_token(SOURCE, &requirement))
            .await
            .unwrap()
    }

    fn stored(&self) -> Option<StoredCredential> {
        self.files.load(SOURCE).unwrap()
    }
}

fn binding(provider: &Provider, key: &str, client: &str) -> OAuthBinding {
    OAuthBinding {
        explicit_configuration: false,
        source: SOURCE.into(),
        configuration: key.into(),
        declared: provider.requirement(),
        client_id: client.into(),
        grant_id: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_an_explicit_default_while_stopped_does_not_restore_implicit_authority() {
    let fixture = fixture().await;
    let configs = Arc::new(Mutex::new(vec![SourceConfiguration {
        id: SOURCE.into(),
        source: SOURCE.into(),
        credential: None,
        oauth_client_id: None,
        options: Default::default(),
    }]));
    let read = configs.clone();
    fixture
        .credentials
        .with_configuration_reader(Arc::new(move || Ok(locked(&read).clone())));
    fixture.sign_in().await;
    let pinned = fixture.stored().unwrap().binding.unwrap();
    assert!(pinned.explicit_configuration);
    locked(&configs).clear();
    // A restarted daemon has no old configuration snapshot from which to infer removal.
    let restarted = Credentials::new(Box::new(FileStore::new(
        fixture._dir.path().join("credentials"),
    )));
    restarted.with_configuration_reader(Arc::new(|| Ok(vec![])));
    assert!(restarted.validate_binding(&pinned).is_err());
    assert!(
        !restarted
            .state(SOURCE, &fixture.provider.requirement())
            .unwrap()
            .signed_in
    );
    assert!(fixture.stored().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn saving_an_implicit_default_preserves_its_grant_but_records_subsequent_removal() {
    let fixture = fixture().await;
    let configs = Arc::new(Mutex::new(vec![]));
    let read = configs.clone();
    fixture
        .credentials
        .with_configuration_reader(Arc::new(move || Ok(locked(&read).clone())));
    fixture.sign_in().await;
    let original = fixture.stored().unwrap().binding.unwrap();
    assert!(!original.explicit_configuration);
    locked(&configs).push(SourceConfiguration {
        id: SOURCE.into(),
        source: SOURCE.into(),
        credential: None,
        oauth_client_id: None,
        options: Default::default(),
    });
    fixture.credentials.validate_binding(&original).unwrap();
    let saved = fixture.stored().unwrap().binding.unwrap();
    assert!(saved.explicit_configuration);
    assert_eq!(saved.grant_id, original.grant_id);
    locked(&configs).clear();
    assert!(fixture.credentials.validate_binding(&original).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_rotation_preserves_a_default_definition_observed_during_provider_io() {
    let fixture = fixture().await;
    fixture.provider.set(|behaviour| {
        behaviour.expires_in = 1;
        behaviour.rotate = true;
    });
    let configs = Arc::new(Mutex::new(vec![]));
    let read = configs.clone();
    fixture
        .credentials
        .with_configuration_reader(Arc::new(move || Ok(locked(&read).clone())));
    fixture.sign_in().await;
    assert!(
        !fixture
            .stored()
            .unwrap()
            .binding
            .unwrap()
            .explicit_configuration
    );
    let changed = configs.clone();
    *fixture.provider.on_refresh.lock().unwrap() = Some(Box::new(move || {
        locked(&changed).push(SourceConfiguration {
            id: SOURCE.into(),
            source: SOURCE.into(),
            credential: None,
            oauth_client_id: None,
            options: Default::default(),
        });
    }));
    fixture.token().await.unwrap();
    let stored = fixture.stored().unwrap();
    assert_eq!(stored.refresh_token.as_deref(), Some("refresh-2"));
    assert!(stored.binding.unwrap().explicit_configuration);
    locked(&configs).clear();
    assert!(
        !fixture
            .credentials
            .state(SOURCE, &fixture.provider.requirement())
            .unwrap()
            .signed_in
    );
}

#[tokio::test]
async fn a_legacy_credential_is_not_adopted_even_when_its_client_and_terms_match() {
    let fixture = fixture().await;
    fixture.sign_in().await;
    let mut legacy = fixture.stored().unwrap();
    legacy.binding = None;
    fixture.files.save(SOURCE, &legacy).unwrap();
    assert!(
        !fixture
            .credentials
            .state(SOURCE, &fixture.provider.requirement())
            .unwrap()
            .signed_in
    );
    assert!(
        fixture.token().await.is_err(),
        "a warm cache cannot revive unbound authority"
    );
    assert!(fixture.stored().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn independent_clients_exchange_and_refresh_without_borrowing_each_others_grants() {
    let fixture = fixture().await;
    fixture.provider.set(|b| {
        b.expires_in = 1;
        b.rotate = true;
    });
    let work = fixture
        .authorize(binding(&fixture.provider, "work", "work-client"))
        .await;
    let mut personal = binding(&fixture.provider, "personal", "personal-client");
    personal.declared = fixture.provider.browser_requirement();
    let personal = fixture.authorize(personal).await;
    assert_ne!(work.grant_id, personal.grant_id);
    for expected in [&work, &personal, &work] {
        let credentials = fixture.credentials.clone();
        let expected = expected.clone();
        tokio::task::spawn_blocking(move || credentials.bound_access_token(&expected))
            .await
            .unwrap()
            .unwrap();
    }
    let restarted = Credentials::new(Box::new(FileStore::new(
        fixture._dir.path().join("credentials"),
    )));
    // The persisted binding survives refresh rotation and process-local cache loss.
    assert_eq!(fixture.credentials.binding(work.clone()).unwrap(), work);
    assert_eq!(
        fixture.files.load("personal").unwrap().unwrap().binding,
        Some(personal.clone())
    );
    assert_eq!(restarted.binding(work.clone()).unwrap(), work);
    let mut borrowed = work.clone();
    borrowed.configuration = "personal".into();
    assert!(fixture.credentials.validate_binding(&borrowed).is_err());
    assert!(fixture.credentials.validate_binding(&personal).is_ok());
    let seen = fixture.provider.seen.lock().unwrap();
    assert!(
        seen.iter()
            .any(
                |form| form.get("grant_type").map(String::as_str) == Some("authorization_code")
                    && form["client_id"] == "personal-client"
            )
    );
    for client in ["work-client", "personal-client"] {
        assert!(
            seen.iter()
                .any(
                    |form| form.get("grant_type").map(String::as_str) == Some("refresh_token")
                        && form["client_id"] == client
                )
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn reauthorization_replaces_a_grant_even_when_the_client_is_unchanged() {
    let fixture = fixture().await;
    let original = binding(&fixture.provider, "work", "work-client");
    let before = fixture.authorize(original.clone()).await;
    let after = fixture.authorize(original).await;
    assert_ne!(before.grant_id, after.grant_id);
    assert!(fixture.credentials.validate_binding(&before).is_err());
    assert!(fixture.credentials.bound_access_token(&before).is_err());
    assert!(fixture.credentials.validate_binding(&after).is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_backend_replacement_cannot_reuse_the_previous_grants_cached_access_token() {
    let mut fixture = fixture().await;
    let original = binding(&fixture.provider, "work", "work-client");
    let before = fixture.authorize(original.clone()).await;
    let previous = fixture.credentials.clone();
    assert_eq!(
        previous.bound_access_token(&before).unwrap().expose(),
        "access-1"
    );
    fixture.credentials = Credentials::new(Box::new(FileStore::new(
        fixture._dir.path().join("credentials"),
    )));
    let after = fixture.authorize(original).await;
    assert!(previous.bound_access_token(&before).is_err());
    let token = tokio::task::spawn_blocking(move || previous.bound_access_token(&after))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(token.expose(), "access-1");
}

#[tokio::test(flavor = "multi_thread")]
async fn observed_client_edits_invalidate_cached_grants_without_touching_other_configurations() {
    let fixture = fixture().await;
    let configs = Arc::new(Mutex::new(vec![
        SourceConfiguration {
            id: "work".into(),
            source: SOURCE.into(),
            credential: None,
            oauth_client_id: Some("work-client".into()),
            options: Default::default(),
        },
        SourceConfiguration {
            id: "personal".into(),
            source: SOURCE.into(),
            credential: None,
            oauth_client_id: Some("personal-client".into()),
            options: Default::default(),
        },
    ]));
    let read = configs.clone();
    fixture
        .credentials
        .with_configuration_reader(Arc::new(move || Ok(locked(&read).clone())));
    let work = fixture
        .authorize(binding(&fixture.provider, "work", "work-client"))
        .await;
    let personal = fixture
        .authorize(binding(&fixture.provider, "personal", "personal-client"))
        .await;
    locked(&configs)[0].oauth_client_id = Some("changed-client".into());
    assert!(fixture.credentials.validate_binding(&work).is_err());
    assert!(fixture.credentials.validate_binding(&personal).is_ok());
    locked(&configs)[0].oauth_client_id = Some("work-client".into());
    assert!(
        fixture.credentials.validate_binding(&work).is_err(),
        "observed changes cannot revive an old grant"
    );
    assert!(fixture.files.load("work").unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_sign_in_keeps_the_credential_and_the_answer_carries_no_token() {
    let fixture = fixture().await;
    let requirement = fixture.provider.requirement();
    let challenge = fixture
        .credentials
        .begin(SOURCE, &requirement)
        .await
        .unwrap();
    assert_eq!(challenge.kind, FlowKind::Device);
    assert_eq!(challenge.user_code.as_deref(), Some("ABCD-1234"));
    let shown = serde_json::to_string(&challenge).unwrap();
    assert!(
        !shown.contains("device-secret"),
        "the device code is the daemon's: {shown}"
    );

    let done = fixture.finish(&challenge.flow).await;
    assert_eq!(done.stored_in, "file");
    assert_eq!(done.scopes, ["read"]);
    let shown = serde_json::to_string(&done).unwrap();
    assert!(
        !shown.contains("access-") && !shown.contains("refresh-"),
        "no token in the answer: {shown}"
    );

    let stored = fixture.stored().expect("the credential is kept");
    assert_eq!(stored.refresh_token.as_deref(), Some("refresh-1"));
    assert_eq!(
        stored.access_token, None,
        "the short-lived token is not persisted when there is a refresh token"
    );
    assert!(fixture.credentials.is_signed_in(SOURCE, &requirement));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_sign_in_waits_through_pending_and_slow_down_answers() {
    let fixture = fixture().await;
    fixture.provider.set(|b| {
        b.pending_polls = 2;
        b.slow_down_once = true;
    });
    let done = fixture.sign_in().await;
    assert!(done.signed_in);
    let polls = fixture
        .provider
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|form| form.contains_key("device_code"))
        .count();
    assert_eq!(polls, 4, "one slow_down, two pending, one answer");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_declined_device_sign_in_ends_with_the_reason_and_keeps_nothing() {
    let fixture = fixture().await;
    fixture.provider.set(|b| b.deny = true);
    let challenge = fixture
        .credentials
        .begin(SOURCE, &fixture.provider.requirement())
        .await
        .unwrap();
    let error = loop {
        match fixture
            .credentials
            .wait(SOURCE, &challenge.flow, Duration::from_millis(100))
            .await
        {
            Ok(FlowStatus::Pending) => {}
            other => break other.unwrap_err(),
        }
    };
    assert!(
        matches!(error, Error::CredentialFlowFailed { .. }),
        "{error}"
    );
    assert!(error.to_string().contains("declined"), "{error}");
    assert!(fixture.stored().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_code_that_expires_before_the_user_finishes_ends_the_sign_in() {
    let fixture = fixture().await;
    fixture.provider.set(|b| {
        b.device_expires_in = 0;
        b.pending_polls = 1_000;
    });
    let challenge = fixture
        .credentials
        .begin(SOURCE, &fixture.provider.requirement())
        .await
        .unwrap();
    let error = loop {
        match fixture
            .credentials
            .wait(SOURCE, &challenge.flow, Duration::from_millis(100))
            .await
        {
            Ok(FlowStatus::Pending) => {}
            other => break other.unwrap_err().to_string(),
        }
    };
    assert!(error.contains("expired"), "{error}");
}

/// Open the callback the way a browser does after the user agrees.
async fn call_back(url: &str, parameters: &[(&str, &str)]) -> reqwest::StatusCode {
    let mut url = reqwest::Url::parse(url).unwrap();
    url.query_pairs_mut().extend_pairs(parameters);
    reqwest::get(url).await.unwrap().status()
}

/// The `state`, redirect URI and PKCE challenge in a browser sign-in's authorization URL.
fn authorization(url: &str) -> std::collections::HashMap<String, String> {
    reqwest::Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_browser_sign_in_exchanges_the_code_with_the_pkce_verifier_that_matches_its_challenge() {
    let fixture = fixture().await;
    let requirement = fixture.provider.browser_requirement();
    let challenge = fixture
        .credentials
        .begin(SOURCE, &requirement)
        .await
        .unwrap();
    assert_eq!(challenge.kind, FlowKind::Browser);
    let url = challenge.url.clone().unwrap();
    let asked = authorization(&url);
    assert_eq!(asked["code_challenge_method"], "S256");
    assert!(
        asked["redirect_uri"].starts_with("http://127.0.0.1:"),
        "{asked:?}"
    );

    let redirect = asked["redirect_uri"].clone();
    let status = call_back(
        &redirect,
        &[("code", "good-code"), ("state", &asked["state"])],
    )
    .await;
    assert_eq!(status, 200);
    let done = fixture.finish(&challenge.flow).await;
    assert!(done.signed_in);

    let seen = fixture.provider.seen.lock().unwrap();
    let exchange = seen.last().unwrap();
    assert_eq!(exchange["grant_type"], "authorization_code");
    assert_eq!(exchange["redirect_uri"], redirect);
    assert_eq!(
        Pkce::challenge_for(&exchange["code_verifier"]),
        asked["code_challenge"],
        "the verifier sent is the one the challenge was made from"
    );
    assert_eq!(exchange["client_id"], "test-client");
    assert!(
        !exchange.contains_key("client_secret"),
        "a public client sends no secret"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_source_declared_callback_path_is_used_by_authorization_and_the_listener() {
    let fixture = fixture().await;
    let mut requirement = fixture.provider.browser_requirement();
    requirement.callback_path = Some("/auth/callback".into());
    let challenge = fixture
        .credentials
        .begin(SOURCE, &requirement)
        .await
        .unwrap();
    let asked = authorization(challenge.url.as_deref().unwrap());
    let redirect = &asked["redirect_uri"];
    assert!(redirect.ends_with("/auth/callback"));

    let wrong = redirect.replace("/auth/callback", "/callback");
    assert_eq!(
        call_back(&wrong, &[("code", "good-code"), ("state", &asked["state"])]).await,
        404
    );
    assert_eq!(
        call_back(
            redirect,
            &[("code", "good-code"), ("state", &asked["state"])]
        )
        .await,
        200
    );
    assert!(fixture.finish(&challenge.flow).await.signed_in);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_callback_with_the_wrong_state_is_refused_and_the_real_one_still_signs_in() {
    let fixture = fixture().await;
    let requirement = fixture.provider.browser_requirement();
    let challenge = fixture
        .credentials
        .begin(SOURCE, &requirement)
        .await
        .unwrap();
    let asked = authorization(&challenge.url.clone().unwrap());
    let redirect = asked["redirect_uri"].clone();

    let forged = call_back(&redirect, &[("code", "good-code"), ("state", "forged")]).await;
    assert_eq!(forged, 400);
    let missing = call_back(&redirect, &[("code", "good-code")]).await;
    assert_eq!(missing, 400);
    assert!(
        fixture.stored().is_none(),
        "a forged callback signs nothing in"
    );

    call_back(
        &redirect,
        &[("code", "good-code"), ("state", &asked["state"])],
    )
    .await;
    assert!(fixture.finish(&challenge.flow).await.signed_in);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_user_who_declines_in_the_browser_ends_the_sign_in() {
    let fixture = fixture().await;
    let requirement = fixture.provider.browser_requirement();
    let challenge = fixture
        .credentials
        .begin(SOURCE, &requirement)
        .await
        .unwrap();
    let asked = authorization(&challenge.url.clone().unwrap());
    call_back(
        &asked["redirect_uri"],
        &[("error", "access_denied"), ("state", &asked["state"])],
    )
    .await;
    let error = fixture
        .credentials
        .wait(SOURCE, &challenge.flow, Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("declined"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_source_that_was_never_signed_in_asks_to_sign_in() {
    let fixture = fixture().await;
    let error = fixture.token().await.unwrap_err();
    assert!(matches!(error, Error::CredentialRequired { .. }), "{error}");
    assert!(
        error.to_string().contains("never been signed in"),
        "{error}"
    );
    let help = miette::Diagnostic::help(&error).unwrap().to_string();
    assert!(help.contains("memcastle source auth demo"), "{help}");
    assert!(
        !fixture
            .credentials
            .is_signed_in(SOURCE, &fixture.provider.requirement())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_with_time_left_is_reused_and_the_provider_is_not_asked() {
    let fixture = fixture().await;
    fixture.sign_in().await;
    let first = fixture.token().await.unwrap();
    let second = fixture.token().await.unwrap();
    assert_eq!(first.expose(), "access-1");
    assert_eq!(second.expose(), "access-1");
    assert_eq!(fixture.provider.refreshes(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_about_to_expire_is_renewed_before_it_is_handed_out_and_the_renewal_is_reused() {
    let fixture = fixture().await;
    // Signed in with a token that lasts 30 seconds, under the renewal margin.
    fixture.provider.set(|b| b.expires_in = 30);
    fixture.sign_in().await;
    fixture.provider.set(|b| b.expires_in = 3600);

    let renewed = fixture.token().await.unwrap();
    assert_eq!(
        renewed.expose(),
        "access-2",
        "a new token, not the one about to expire"
    );
    assert_eq!(fixture.provider.refreshes(), 1);
    assert_eq!(fixture.token().await.unwrap().expose(), "access-2");
    assert_eq!(
        fixture.provider.refreshes(),
        1,
        "the renewal lasts, so it is not renewed again"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_credential_kept_by_an_earlier_daemon_is_renewed_after_a_restart() {
    let fixture = fixture().await;
    fixture.sign_in().await;
    // A new daemon: the same files, and nothing in memory.
    let restarted = Credentials::new(Box::new(FileStore::new(
        fixture._dir.path().join("credentials"),
    )));
    let requirement = fixture.provider.requirement();
    assert!(restarted.is_signed_in(SOURCE, &requirement));
    let token = tokio::task::spawn_blocking(move || restarted.access_token(SOURCE, &requirement))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(token.expose(), "access-2");
    assert_eq!(fixture.provider.refresh_tokens_seen(), ["refresh-1"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn many_callers_at_once_share_one_renewal() {
    let fixture = fixture().await;
    fixture.provider.set(|b| b.expires_in = 30);
    fixture.sign_in().await;
    fixture.provider.set(|b| b.expires_in = 3600);

    let calls: Vec<_> = (0..8)
        .map(|_| {
            let credentials = fixture.credentials.clone();
            let requirement = fixture.provider.requirement();
            tokio::task::spawn_blocking(move || credentials.access_token(SOURCE, &requirement))
        })
        .collect();
    for call in calls {
        assert_eq!(call.await.unwrap().unwrap().expose(), "access-2");
    }
    assert_eq!(
        fixture.provider.refreshes(),
        1,
        "two renewals would use a rotating refresh token twice, which a provider answers by revoking it"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rotated_refresh_token_replaces_the_stored_one_and_is_what_the_next_renewal_uses() {
    let fixture = fixture().await;
    fixture.provider.set(|b| {
        b.expires_in = 30;
        b.rotate = true;
    });
    fixture.sign_in().await;
    assert_eq!(
        fixture.stored().unwrap().refresh_token.as_deref(),
        Some("refresh-1")
    );

    fixture.token().await.unwrap();
    assert_eq!(
        fixture.stored().unwrap().refresh_token.as_deref(),
        Some("refresh-2")
    );
    fixture.token().await.unwrap();
    assert_eq!(
        fixture.provider.refresh_tokens_seen(),
        ["refresh-1", "refresh-2"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_renewal_that_rotates_nothing_does_not_rewrite_the_store() {
    let fixture = fixture().await;
    fixture.provider.set(|b| b.expires_in = 30);
    fixture.sign_in().await;
    let before = fixture.stored().unwrap();
    fixture.token().await.unwrap();
    assert_eq!(fixture.stored().unwrap(), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_credential_the_provider_revoked_is_removed_and_asks_to_sign_in_again() {
    let fixture = fixture().await;
    fixture.provider.set(|b| b.expires_in = 30);
    fixture.sign_in().await;
    fixture.provider.set(|b| b.revoke = true);

    let error = fixture.token().await.unwrap_err();
    assert!(matches!(error, Error::CredentialRequired { .. }), "{error}");
    assert!(error.to_string().contains("no longer honours"), "{error}");
    assert!(
        fixture.stored().is_none(),
        "a credential that can never work again is not kept"
    );
    assert!(
        !fixture
            .credentials
            .is_signed_in(SOURCE, &fixture.provider.requirement())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_provider_outage_keeps_the_credential_and_the_next_attempt_succeeds() {
    let fixture = fixture().await;
    fixture.provider.set(|b| b.expires_in = 30);
    fixture.sign_in().await;
    fixture.provider.set(|b| {
        b.outage = true;
        b.expires_in = 3600;
    });

    let error = fixture.token().await.unwrap_err();
    assert!(
        matches!(error, Error::CredentialRefreshFailed { .. }),
        "{error}"
    );
    assert!(fixture.stored().is_some(), "an outage is not a sign-out");

    fixture.provider.set(|b| b.outage = false);
    assert_eq!(fixture.token().await.unwrap().expose(), "access-2");
}

#[tokio::test(flavor = "multi_thread")]
async fn signing_in_again_replaces_the_credential_and_an_older_attempt() {
    let fixture = fixture().await;
    fixture.sign_in().await;
    assert_eq!(
        fixture.stored().unwrap().refresh_token.as_deref(),
        Some("refresh-1")
    );

    let requirement = fixture.provider.requirement();
    fixture.provider.set(|b| b.pending_polls = 1_000);
    let abandoned = fixture
        .credentials
        .begin(SOURCE, &requirement)
        .await
        .unwrap();
    // Begun while the first is still waiting, so the first cannot finish in between.
    let again = fixture
        .credentials
        .begin(SOURCE, &requirement)
        .await
        .unwrap();
    fixture.provider.set(|b| b.pending_polls = 0);
    assert_ne!(abandoned.flow, again.flow);

    let error = fixture
        .credentials
        .wait(SOURCE, &abandoned.flow, Duration::from_millis(10))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("no such sign-in"), "{error}");

    assert!(fixture.finish(&again.flow).await.signed_in);
    assert_eq!(
        fixture.stored().unwrap().refresh_token.as_deref(),
        Some("refresh-2")
    );
    assert_eq!(fixture.token().await.unwrap().expose(), "access-2");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_credential_obtained_under_other_terms_is_treated_as_missing() {
    let fixture = fixture().await;
    fixture.sign_in().await;

    let mut wider = fixture.provider.requirement();
    wider.scopes.push("admin".into());
    assert!(!fixture.credentials.is_signed_in(SOURCE, &wider));
    let error = fixture.token_for(wider.clone()).await.unwrap_err();
    assert!(matches!(error, Error::CredentialRequired { .. }), "{error}");
    assert!(error.to_string().contains("other terms"), "{error}");
    assert!(!fixture.credentials.state(SOURCE, &wider).unwrap().signed_in);
    // The old terms still work: the credential was not destroyed by being asked about under new ones.
    assert!(fixture.token().await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_provider_that_issues_no_refresh_token_is_used_until_its_token_expires() {
    let fixture = fixture().await;
    fixture.provider.set(|b| b.issue_refresh = false);
    fixture.sign_in().await;
    let stored = fixture.stored().unwrap();
    assert!(
        stored.refresh_token.is_none()
            && stored.access_token.is_some()
            && stored.expires_at.is_some()
    );
    assert_eq!(fixture.token().await.unwrap().expose(), "access-1");

    // A restarted daemon has only the file, whose token is still valid.
    let restarted = Credentials::new(Box::new(FileStore::new(
        fixture._dir.path().join("credentials"),
    )));
    let requirement = fixture.provider.requirement();
    let token = tokio::task::spawn_blocking(move || restarted.access_token(SOURCE, &requirement))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(token.expose(), "access-1");
    assert_eq!(fixture.provider.refreshes(), 0);

    // Once it has expired there is nothing to renew it with.
    fixture.provider.set(|b| b.expires_in = 30);
    let other = Credentials::new(Box::new(FileStore::new(
        fixture._dir.path().join("credentials"),
    )));
    let mut stale = stored;
    stale.expires_at = Some(Utc::now() - TimeDelta::seconds(5));
    fixture.files.save(SOURCE, &stale).unwrap();
    let requirement = fixture.provider.requirement();
    let error = tokio::task::spawn_blocking(move || other.access_token(SOURCE, &requirement))
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(error, Error::CredentialRequired { .. }), "{error}");
    assert!(error.to_string().contains("no refresh token"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn forgetting_a_source_removes_its_credential_and_its_sign_in_in_progress() {
    let fixture = fixture().await;
    fixture.sign_in().await;
    fixture.provider.set(|b| b.pending_polls = 1_000);
    let pending = fixture
        .credentials
        .begin(SOURCE, &fixture.provider.requirement())
        .await
        .unwrap();

    fixture.credentials.forget(SOURCE).unwrap();
    assert!(fixture.stored().is_none());
    assert!(
        !fixture
            .credentials
            .is_signed_in(SOURCE, &fixture.provider.requirement())
    );
    let error = fixture
        .credentials
        .wait(SOURCE, &pending.flow, Duration::from_millis(10))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("no such sign-in"), "{error}");
    // Forgetting what is not there is fine.
    fixture.credentials.forget(SOURCE).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_state_of_a_signed_in_source_says_what_was_granted_and_carries_no_token() {
    let fixture = fixture().await;
    let requirement = fixture.provider.requirement();
    assert!(
        !fixture
            .credentials
            .state(SOURCE, &requirement)
            .unwrap()
            .signed_in
    );
    fixture.sign_in().await;
    let state = fixture.credentials.state(SOURCE, &requirement).unwrap();
    assert!(state.signed_in && state.expires_at.is_some());
    assert_eq!(state.scopes, ["read"]);
    let shown = serde_json::to_string(&state).unwrap();
    assert!(
        !shown.contains("access-") && !shown.contains("refresh-"),
        "{shown}"
    );
    assert_eq!(fixture.provider.refresh_calls.load(Ordering::SeqCst), 0);
}
