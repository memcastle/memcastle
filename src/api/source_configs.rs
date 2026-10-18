//! Source configuration resources: administrative writes and token-free reads.

use axum::extract::{Path, State};
use axum::http::HeaderValue;
use axum::http::header::CACHE_CONTROL;
use axum::response::Response;
use axum::response::{IntoResponse, Json};

use super::extract::ApiJson;
use super::{ApiError, ApiState, ModeHeader};
use crate::app::SourceConfigurationPatch;

pub(super) async fn auth_begin(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mut answer = Json(state.app.begin_source_config_auth(&id).await?).into_response();
    answer
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(answer)
}

#[derive(serde::Deserialize)]
pub(super) struct WaitQuery {
    flow: String,
    #[serde(default)]
    seconds: Option<u64>,
}

pub(super) async fn auth_wait(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    super::extract::ApiQuery(query): super::extract::ApiQuery<WaitQuery>,
) -> Result<Response, ApiError> {
    let state = state
        .app
        .wait_source_config_auth(
            &id,
            &query.flow,
            std::time::Duration::from_secs(query.seconds.unwrap_or(30)),
        )
        .await?;
    let mut answer = Json(state).into_response();
    answer
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(answer)
}

pub(super) async fn list(
    State(state): State<ApiState>,
    ModeHeader(mode): ModeHeader,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(state.app.list_source_configs(mode).await?))
}

pub(super) async fn show(
    State(state): State<ApiState>,
    ModeHeader(mode): ModeHeader,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(state.app.show_source_config(&id, mode).await?))
}

pub(super) async fn set(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    ApiJson(patch): ApiJson<SourceConfigurationPatch>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(state.app.set_source_config(&id, patch).await?))
}

pub(super) async fn remove(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    state.app.remove_source_config(&id).await?;
    Ok(Json(serde_json::json!({ "removed": id })))
}
