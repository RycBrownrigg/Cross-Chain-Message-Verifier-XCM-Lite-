use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// High-level error codes exposed by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum XcmErrorCode {
    InvalidPayload,
    InvalidSignature,
    VersionMismatch,
    UnsupportedInstruction,
}

impl Display for XcmErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            XcmErrorCode::InvalidPayload => "InvalidPayload",
            XcmErrorCode::InvalidSignature => "InvalidSignature",
            XcmErrorCode::VersionMismatch => "VersionMismatch",
            XcmErrorCode::UnsupportedInstruction => "UnsupportedInstruction",
        })
    }
}

/// Validation error containing a code and human-readable detail.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
#[error("{code}: {detail}")]
pub struct MessageValidationError {
    pub code: XcmErrorCode,
    pub detail: String,
}

impl MessageValidationError {
    pub fn invalid_payload(detail: impl Into<String>) -> Self {
        Self {
            code: XcmErrorCode::InvalidPayload,
            detail: detail.into(),
        }
    }

    pub fn unsupported_instruction(detail: impl Into<String>) -> Self {
        Self {
            code: XcmErrorCode::UnsupportedInstruction,
            detail: detail.into(),
        }
    }

    pub fn version_mismatch(detail: impl Into<String>) -> Self {
        Self {
            code: XcmErrorCode::VersionMismatch,
            detail: detail.into(),
        }
    }
}
