//! Health-check handlers (no service layer).

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::{schemas::health::HealthResponse, state::AppState};

/// `GET /health` — liveness. Always 200 with version/commit metadata.
#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Service is alive", body = HealthResponse)),
    tag = "health"
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse::ok(
        state.settings.app_version.clone(),
        state.settings.git_commit.clone(),
    ))
}

/// `GET /healthz/db` — readiness. Pings the pool; 200 on success, 503 otherwise.
#[utoipa::path(
    get,
    path = "/healthz/db",
    responses(
        (status = 200, description = "Database reachable", body = HealthResponse),
        (status = 503, description = "Database unreachable")
    ),
    tag = "health"
)]
pub async fn healthz_db(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok".to_string(),
                version: None,
                git_commit: None,
            }),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = ?err, "db health check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "unavailable".to_string(),
                    version: None,
                    git_commit: None,
                }),
            )
                .into_response()
        }
    }
}
