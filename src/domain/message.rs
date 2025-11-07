use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

use super::errors::{MessageValidationError, XcmErrorCode};

/// Supported XCM versions for the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum XcmVersion {
    V3,
    V4,
}

impl XcmVersion {
    pub fn is_supported(self, configured: &str) -> bool {
        let normalized = configured.trim().to_uppercase();
        match self {
            XcmVersion::V3 => normalized == "V3",
            XcmVersion::V4 => normalized == "V4",
        }
    }
}

impl FromStr for XcmVersion {
    type Err = MessageValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_uppercase().as_str() {
            "V3" => Ok(XcmVersion::V3),
            "V4" => Ok(XcmVersion::V4),
            other => Err(MessageValidationError::version_mismatch(format!(
                "unsupported XCM version: {other}"
            ))),
        }
    }
}

impl Display for XcmVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XcmVersion::V3 => write!(f, "V3"),
            XcmVersion::V4 => write!(f, "V4"),
        }
    }
}

/// Envelope representing an incoming message submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEnvelope {
    pub message_id: Option<String>,
    pub sender_para: u32,
    pub dest_para: u32,
    pub xcm_version: XcmVersion,
    pub instructions: Vec<Instruction>,
    #[serde(default)]
    pub signature: Option<String>,
}

impl MessageEnvelope {
    /// Validate structural correctness and supported features.
    pub fn validate(&self, configured_version: &str) -> Result<(), MessageValidationError> {
        if self.sender_para == 0 || self.dest_para == 0 {
            return Err(MessageValidationError::invalid_payload(
                "sender and destination parachain IDs must be non-zero",
            ));
        }

        if self.sender_para == self.dest_para {
            return Err(MessageValidationError::invalid_payload(
                "sender and destination parachain IDs must differ",
            ));
        }

        if self.instructions.is_empty() {
            return Err(MessageValidationError::invalid_payload(
                "at least one instruction is required",
            ));
        }

        if !self.xcm_version.is_supported(configured_version) {
            return Err(MessageValidationError {
                code: XcmErrorCode::VersionMismatch,
                detail: format!(
                    "message version {0} mismatches configured version {configured_version}",
                    self.xcm_version
                ),
            });
        }

        for (idx, instruction) in self.instructions.iter().enumerate() {
            instruction.validate().map_err(|err| {
                MessageValidationError::invalid_payload(format!(
                    "instruction {idx} invalid: {}",
                    err.detail
                ))
            })?;
        }

        Ok(())
    }
}

/// Supported instruction set for the MVP.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Instruction {
    TransferReserveAsset(TransferReserveAsset),
    Transact(Transact),
    QueryResponse(QueryResponse),
}

impl Instruction {
    pub fn validate(&self) -> Result<(), MessageValidationError> {
        match self {
            Instruction::TransferReserveAsset(data) => data.validate(),
            Instruction::Transact(data) => data.validate(),
            Instruction::QueryResponse(data) => data.validate(),
        }
    }
}

/// Representation of a `TransferReserveAsset` instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferReserveAsset {
    pub asset: String,
    pub amount: u128,
    pub beneficiary: String,
}

impl TransferReserveAsset {
    fn validate(&self) -> Result<(), MessageValidationError> {
        if self.asset.trim().is_empty() {
            return Err(MessageValidationError::invalid_payload(
                "asset identifier must be provided",
            ));
        }
        if self.amount == 0 {
            return Err(MessageValidationError::invalid_payload(
                "transfer amount must be greater than zero",
            ));
        }
        if self.beneficiary.trim().is_empty() {
            return Err(MessageValidationError::invalid_payload(
                "beneficiary must be provided",
            ));
        }
        Ok(())
    }
}

/// Representation of a `Transact` instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transact {
    pub call_data: String,
    #[serde(default)]
    pub weight: Option<u64>,
}

impl Transact {
    fn validate(&self) -> Result<(), MessageValidationError> {
        if self.call_data.trim().is_empty() {
            return Err(MessageValidationError::invalid_payload(
                "call_data must be provided",
            ));
        }
        Ok(())
    }
}

/// Representation of a `QueryResponse` instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResponse {
    pub query_id: String,
    pub response: String,
}

impl QueryResponse {
    fn validate(&self) -> Result<(), MessageValidationError> {
        if self.query_id.trim().is_empty() {
            return Err(MessageValidationError::invalid_payload(
                "query_id must be provided",
            ));
        }
        if self.response.trim().is_empty() {
            return Err(MessageValidationError::invalid_payload(
                "response must be provided",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message() -> MessageEnvelope {
        MessageEnvelope {
            message_id: Some("msg-1".to_string()),
            sender_para: 1000,
            dest_para: 2000,
            xcm_version: XcmVersion::V3,
            instructions: vec![Instruction::TransferReserveAsset(TransferReserveAsset {
                asset: "DOT".into(),
                amount: 10,
                beneficiary: "acct-123".into(),
            })],
            signature: Some("deadbeef".into()),
        }
    }

    #[test]
    fn validates_correct_message() {
        let message = sample_message();
        assert!(message.validate("V3").is_ok());
    }

    #[test]
    fn rejects_missing_instructions() {
        let mut message = sample_message();
        message.instructions.clear();
        let err = message.validate("V3").unwrap_err();
        assert_eq!(err.code, XcmErrorCode::InvalidPayload);
    }

    #[test]
    fn rejects_version_mismatch() {
        let message = sample_message();
        let err = message.validate("V4").unwrap_err();
        assert_eq!(err.code, XcmErrorCode::VersionMismatch);
    }
}
