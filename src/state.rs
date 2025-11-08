use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serde::Serialize;
use thiserror::Error;

use crate::config::ParachainConfig;

/// Shared, concurrent state for the XCM Lite service.
#[derive(Clone)]
pub struct ServiceState {
    pub parachains: Arc<RwLock<HashMap<u32, ParachainState>>>,
    pub messages: Arc<RwLock<HashMap<String, MessageRecord>>>,
}

impl ServiceState {
    /// Initialise state structures based on configuration.
    pub fn initialize(config: &ParachainConfig) -> Result<Self, StateInitError> {
        let mut parachains = HashMap::new();
        for para_id in config.parachain_ids() {
            if parachains
                .insert(para_id, ParachainState::default())
                .is_some()
            {
                return Err(StateInitError::DuplicateParaId(para_id));
            }
        }

        Ok(Self {
            parachains: Arc::new(RwLock::new(parachains)),
            messages: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Return the count of currently registered parachains.
    pub fn parachain_count(&self) -> usize {
        self.parachains.read().map(|map| map.len()).unwrap_or(0)
    }
}

impl Default for ServiceState {
    fn default() -> Self {
        Self {
            parachains: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// State associated with a single parachain in the simulation.
#[derive(Debug, Clone, Default)]
pub struct ParachainState {
    pub balances: HashMap<String, u128>,
    pub logs: Vec<String>,
}

/// Record tracking the lifecycle of a submitted XCM message.
#[derive(Debug, Clone, Serialize)]
pub struct MessageRecord {
    pub status: MessageStatus,
    pub hops: Vec<u32>,
}

impl Default for MessageRecord {
    fn default() -> Self {
        Self {
            status: MessageStatus::Pending,
            hops: Vec::new(),
        }
    }
}

/// High-level message processing status values.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum MessageStatus {
    Pending,
    Relayed,
    Executed { outcome: Option<String> },
    Failed { error: String },
}

impl Default for MessageStatus {
    fn default() -> Self {
        MessageStatus::Pending
    }
}

/// Errors that can occur while initialising state.
#[derive(Debug, Error)]
pub enum StateInitError {
    #[error("duplicate parachain id detected: {0}")]
    DuplicateParaId(u32),
}
