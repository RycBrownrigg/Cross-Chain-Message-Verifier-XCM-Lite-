use std::sync::Arc;

use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;

use crate::{
    crypto::KeyRegistry,
    domain::{MessageEnvelope, MessageValidationError},
    execution::ExecutionEngine,
    state::{MessageRecord, MessageStatus, ServiceState},
};

/// Maximum number of hops supported by the relay.
const MAX_HOPS: usize = 3;

/// Message stored in the processing queue.
#[derive(Debug)]
pub struct QueuedMessage {
    pub message_id: String,
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
    ) -> (Arc<Self>, Receiver<QueuedMessage>) {
        let (sender, receiver) = mpsc::channel(128);
        let processor = Arc::new(Self {
            state,
            keys,
            configured_version: configured_version.into(),
            sender,
        });
        (processor, receiver)
    }

    /// Validate message payload, ensure the signature is correct, and enqueue for relay.
    pub async fn submit_message(
        &self,
        envelope: MessageEnvelope,
        raw_payload: Vec<u8>,
        signature: &[u8],
    ) -> Result<String, ProcessorError> {
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

        let response_id = message_id.clone();
        let queued = QueuedMessage {
            message_id,
            envelope,
            raw_payload,
        };

        self.sender
            .send(queued)
            .await
            .map_err(|_| ProcessorError::ChannelClosed)?;

        Ok(response_id)
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
pub async fn run_relay_loop(
    state: ServiceState,
    engine: Arc<dyn ExecutionEngine>,
    mut receiver: Receiver<QueuedMessage>,
) {
    while let Some(queued) = receiver.recv().await {
        let message_id = queued.message_id.clone();
        let hops = vec![queued.envelope.sender_para, queued.envelope.dest_para];

        let status = if hops.len() > MAX_HOPS {
            MessageStatus::Failed {
                error: "maximum hop count exceeded".to_string(),
            }
        } else {
            match engine.execute(&queued.envelope) {
                Ok(outcome) => MessageStatus::Executed {
                    outcome: outcome.summary(),
                },
                Err(err) => MessageStatus::Failed {
                    error: err.to_string(),
                },
            }
        };

        let mut messages = match state.messages.write() {
            Ok(guard) => guard,
            Err(_) => continue,
        };

        if let Some(record) = messages.get_mut(&message_id) {
            record.status = status;
            record.hops = hops.clone();
        } else {
            messages.insert(message_id, MessageRecord { status, hops });
        }
    }
}
