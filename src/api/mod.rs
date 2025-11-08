use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    config::AppConfig,
    domain::{MessageEnvelope, MessageValidationError, XcmErrorCode},
    processor::{MessageProcessor, ProcessorError},
    state::{MessageRecord, ServiceState},
};

#[derive(Clone)]
pub struct ApiContext {
    pub config: Arc<AppConfig>,
    pub state: ServiceState,
    pub processor: Arc<MessageProcessor>,
}

pub fn router(context: ApiContext) -> Router {
    Router::new()
        .route("/submit", post(submit_message))
        .route("/status/:id", get(get_status))
        .route("/config", get(get_config))
        .with_state(context)
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Validation(#[from] MessageValidationError),
    #[error(transparent)]
    Processing(#[from] ProcessorError),
    #[error("message not found")]
    NotFound,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            ApiError::Validation(err) => match err.code {
                XcmErrorCode::InvalidPayload => StatusCode::BAD_REQUEST,
                XcmErrorCode::VersionMismatch => StatusCode::CONFLICT,
                XcmErrorCode::UnsupportedInstruction => StatusCode::BAD_REQUEST,
                XcmErrorCode::InvalidSignature => StatusCode::UNAUTHORIZED,
            },
            ApiError::Processing(detail) => match detail {
                ProcessorError::Validation(_) => StatusCode::BAD_REQUEST,
                ProcessorError::Signature(_) => StatusCode::UNAUTHORIZED,
                ProcessorError::ChannelClosed => StatusCode::SERVICE_UNAVAILABLE,
                ProcessorError::StatePoisoned => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ApiError::NotFound => StatusCode::NOT_FOUND,
        };

        let body = Json(ErrorResponse::from(&self));
        (status, body).into_response()
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: XcmErrorCode,
    message: String,
}

impl ErrorResponse {
    fn from(error: &ApiError) -> Self {
        match error {
            ApiError::Validation(err) => Self {
                code: err.code,
                message: err.detail.clone(),
            },
            ApiError::Processing(err) => match err {
                ProcessorError::Validation(inner) => Self {
                    code: inner.code,
                    message: inner.detail.clone(),
                },
                ProcessorError::Signature(_) => Self {
                    code: XcmErrorCode::InvalidSignature,
                    message: "signature verification failed".into(),
                },
                ProcessorError::ChannelClosed => Self {
                    code: XcmErrorCode::InvalidPayload,
                    message: "relay channel unavailable".into(),
                },
                ProcessorError::StatePoisoned => Self {
                    code: XcmErrorCode::InvalidPayload,
                    message: "internal state error".into(),
                },
            },
            ApiError::NotFound => Self {
                code: XcmErrorCode::InvalidPayload,
                message: "message not found".into(),
            },
        }
    }
}

async fn submit_message(
    State(context): State<ApiContext>,
    Json(payload): Json<MessageEnvelope>,
) -> Result<Json<SubmitResponse>, ApiError> {
    let raw_payload = serde_json::to_vec(&payload).map_err(|err| {
        MessageValidationError::invalid_payload(format!("serialization error: {err}"))
    })?;

    let signature = payload
        .signature
        .as_ref()
        .ok_or_else(|| MessageValidationError::invalid_payload("signature is required"))?;

    let signature_bytes = hex::decode(signature).map_err(|err| {
        MessageValidationError::invalid_payload(format!("signature decoding failed: {err}"))
    })?;

    let message_id = context
        .processor
        .submit_message(payload, raw_payload, &signature_bytes)
        .await?;

    Ok(Json(SubmitResponse {
        status: "Accepted".into(),
        message_id,
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitResponse {
    status: String,
    message_id: String,
}

async fn get_status(
    State(context): State<ApiContext>,
    Path(id): Path<String>,
) -> Result<Json<MessageRecord>, ApiError> {
    let guard = context
        .state
        .messages
        .read()
        .map_err(|_| ProcessorError::StatePoisoned)?;
    guard.get(&id).cloned().map(Json).ok_or(ApiError::NotFound)
}

async fn get_config(State(context): State<ApiContext>) -> Json<AppConfig> {
    Json((*context.config).clone())
}
