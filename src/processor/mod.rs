use tokio::sync::mpsc::{self, Receiver, Sender};

use uuid::Uuid;

use crate::{
    crypto::KeyRegistry,
    domain::{MessageEnvelope, MessageValidationError},
    state::{MessageRecord, MessageStatus, ServiceState},
};

/// Maximum number of hops supported by the relay.
const MAX_HOPS: usize = 3;

/// Message stored in the processing queue.
#[derive(Debug)]
pub struct QueuedMessage {
    pub envelope: MessageEnvelope,
    pub raw_payload: Vec<u8>,
}

/// Coordinates message validation, signature checking, and routing through the simulated relay.
pub struct MessageProcessor {
    state: ServiceState,
    keys: KeyRegistry,
    configured_version: String,
    sender: Sender<QueuedMessage>,
}

impl MessageProcessor {
    pub fn new(
        state: ServiceState,
        keys: KeyRegistry,
        configured_version: impl Into<String>,
    ) -> (Self, Receiver<QueuedMessage>) {
        let (sender, receiver) = mpsc::channel(128);
        (
            Self {
                state,
                keys,
                configured_version: configured_version.into(),
                sender,
            },
            receiver,
        )
    }

    /// Validate message payload, ensure the signature is correct, and enqueue for relay.
    pub async fn submit_message(
        &self,
        envelope: MessageEnvelope,
        raw_payload: Vec<u8>,
        signature: &[u8],
    ) -> Result<(), ProcessorError> {
        envelope.validate(&self.configured_version)?;
        self.keys
            .verify_signature(envelope.sender_para, &raw_payload, signature)?;

        let message_id = envelope
            .message_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        {
            let mut messages = self
                .state
                .messages
                .write()
                .map_err(|_| ProcessorError::StatePoisoned)?;
            messages.insert(
                message_id.clone(),
                MessageRecord {
                    status: MessageStatus::Pending,
                    hops: vec![envelope.sender_para],
                },
            );
        }

        self.sender
            .send(QueuedMessage {
                envelope,
                raw_payload,
            })
            .await
            .map_err(|_| ProcessorError::ChannelClosed)
    }
}

/// Errors that can occur while processing a message submission.
#[derive(Debug, thiserror::Error)]
pub enum ProcessorError {
    #[error(transparent)]
    Validation(#[from] MessageValidationError),
    #[error(transparent)]
    Signature(#[from] crate::crypto::CryptoError),
    #[error("relay channel closed")]
    ChannelClosed,
    #[error("state lock poisoned")]
    StatePoisoned,
}

/// Run the relay loop, routing queued messages through simulated hops.
pub async fn run_relay_loop(state: ServiceState, mut receiver: Receiver<QueuedMessage>) {
    while let Some(mut queued) = receiver.recv().await {
        let message_id = queued
            .envelope
            .message_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let result = process_single_message(&state, &mut queued);

        let new_status = match result {
            Ok(()) => MessageStatus::Relayed,
            Err(err) => MessageStatus::Failed {
                error: err.to_string(),
            },
        };

        let mut messages = match state.messages.write() {
            Ok(guard) => guard,
            Err(_) => continue,
        };

        if let Some(record) = messages.get_mut(&message_id) {
            record.status = new_status;
        } else {
            messages.insert(
                message_id,
                MessageRecord {
                    status: new_status,
                    hops: vec![],
                },
            );
        }
    }
}

fn process_single_message(
    state: &ServiceState,
    queued: &mut QueuedMessage,
) -> Result<(), RelayError> {
    let mut hops = Vec::new();
    hops.push(queued.envelope.sender_para);
    hops.push(queued.envelope.dest_para);

    if hops.len() > MAX_HOPS {
        return Err(RelayError::HopLimitExceeded);
    }

    {
        let mut messages = state
            .messages
            .write()
            .map_err(|_| RelayError::StatePoisoned)?;
        let message_id = queued
            .envelope
            .message_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        if let Some(record) = messages.get_mut(&message_id) {
            record.hops = hops.clone();
        }
    }

    let mut dest_state = state
        .parachains
        .write()
        .map_err(|_| RelayError::StatePoisoned)?;

    let Some(parachain) = dest_state.get_mut(&queued.envelope.dest_para) else {
        return Err(RelayError::UnknownDestination {
            para_id: queued.envelope.dest_para,
        });
    };

    parachain.logs.push(format!(
        "Received message with {} instructions",
        queued.envelope.instructions.len()
    ));

    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum RelayError {
    #[error("destination parachain {para_id} not found")]
    UnknownDestination { para_id: u32 },
    #[error("state lock poisoned")]
    StatePoisoned,
    #[error("maximum hop count exceeded")]
    HopLimitExceeded,
}
