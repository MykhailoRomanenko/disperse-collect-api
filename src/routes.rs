use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::Serialize;
use std::{borrow::Cow, sync::Arc};

use crate::{
    dto::{
        CollectErc20Request, CollectErc20Response, DisperseErc20Request, DisperseErc20Response,
        DisperseEthRequest, DisperseEthResponse,
    },
    service::{self, DcError}, state::AppState,
};

type Result<T> = std::result::Result<Json<T>, ApiError>;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("unexpected error: {0}")]
    Internal(#[source] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        #[derive(Serialize)]
        struct ErrorResponse<'a> {
            code: u16,
            message: Cow<'a, str>,
        }

        let (message, code) = match self {
            ApiError::InvalidRequest(s) => (Cow::Owned(s), StatusCode::BAD_REQUEST),

            ApiError::Internal(_) => (
                "internal server error".into(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        };

        Json(ErrorResponse {
            code: code.as_u16(),
            message,
        })
        .into_response()
    }
}

impl From<DcError> for ApiError {
    fn from(value: DcError) -> Self {
        match value {
            e @ DcError::InsufficientFunds { .. }
            | e @ DcError::InvalidFractionalAmount(_)
            | e @ DcError::TokenNotFound(_) => Self::InvalidRequest(e.to_string()),
            e => Self::Internal(e.into()),
        }
    }
}

pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/disperse-eth", post(handle_disperse_eth))
        .route("/disperse-erc20", post(handle_disperse_erc20))
        .route("/collect-erc20", post(handle_collect_erc20))
        .with_state(state)
}

async fn handle_disperse_eth(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DisperseEthRequest>,
) -> Result<DisperseEthResponse> {
    service::disperse_eth(state.provider(), state.contract(), req)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn handle_disperse_erc20(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DisperseErc20Request>,
) -> Result<DisperseErc20Response> {
    service::disperse_erc20(state.provider(), state.contract(), req)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn handle_collect_erc20(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CollectErc20Request>,
) -> Result<CollectErc20Response> {
    service::collect_erc20(state.provider(), state.contract(), req)
        .await
        .map(Json)
        .map_err(Into::into)
}
