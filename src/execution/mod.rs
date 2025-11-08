use crate::{
    domain::{Instruction, MessageEnvelope, QueryResponse, Transact, TransferReserveAsset},
    state::{ParachainState, ServiceState},
};
use thiserror::Error;

/// Trait describing message execution behaviour for simulated parachains.
pub trait ExecutionEngine: Send + Sync {
    fn execute(&self, message: &MessageEnvelope) -> Result<ExecutionOutcome, ExecutionError>;
}

/// Outcome details produced by the execution engine.
#[derive(Debug, Clone)]
pub struct ExecutionOutcome {
    pub logs: Vec<String>,
}

impl ExecutionOutcome {
    pub fn summary(&self) -> Option<String> {
        if self.logs.is_empty() {
            None
        } else {
            Some(format!("{} instructions applied", self.logs.len()))
        }
    }
}

/// Default implementation applying mock effects to in-memory state.
pub struct DefaultExecutionEngine {
    state: ServiceState,
}

impl DefaultExecutionEngine {
    pub fn new(state: ServiceState) -> Self {
        Self { state }
    }
}

impl ExecutionEngine for DefaultExecutionEngine {
    fn execute(&self, message: &MessageEnvelope) -> Result<ExecutionOutcome, ExecutionError> {
        let mut parachains = self
            .state
            .parachains
            .write()
            .map_err(|_| ExecutionError::StatePoisoned)?;

        let dest_state =
            parachains
                .get_mut(&message.dest_para)
                .ok_or(ExecutionError::UnknownParachain {
                    para_id: message.dest_para,
                })?;

        let mut logs = Vec::new();

        for instruction in &message.instructions {
            match instruction {
                Instruction::TransferReserveAsset(data) => {
                    apply_transfer(dest_state, data);
                    logs.push(format!(
                        "TransferReserveAsset: {} {} to {}",
                        data.amount, data.asset, data.beneficiary
                    ));
                }
                Instruction::Transact(data) => {
                    apply_transact(dest_state, data);
                    logs.push(format!(
                        "Transact: call_data={} bytes, weight={}",
                        data.call_data.len(),
                        data.weight.unwrap_or_default()
                    ));
                }
                Instruction::QueryResponse(data) => {
                    apply_query(dest_state, data);
                    logs.push(format!(
                        "QueryResponse: id={}, response_length={}",
                        data.query_id,
                        data.response.len()
                    ));
                }
            }
        }

        Ok(ExecutionOutcome { logs })
    }
}

fn apply_transfer(state: &mut ParachainState, transfer: &TransferReserveAsset) {
    let entry = state
        .balances
        .entry(transfer.beneficiary.clone())
        .or_insert(0);
    *entry = entry.saturating_add(transfer.amount);
    state.logs.push(format!(
        "Balance updated: {} => {}",
        transfer.beneficiary, *entry
    ));
}

fn apply_transact(state: &mut ParachainState, transact: &Transact) {
    state.logs.push(format!(
        "Transact executed: call_data_len={}, weight={}",
        transact.call_data.len(),
        transact.weight.unwrap_or_default()
    ));
}

fn apply_query(state: &mut ParachainState, response: &QueryResponse) {
    state.logs.push(format!(
        "QueryResponse stored: id={}, response={}",
        response.query_id, response.response
    ));
}

/// Execution errors surfaced to the processor.
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("destination parachain {para_id} not registered")]
    UnknownParachain { para_id: u32 },
    #[error("state lock poisoned")]
    StatePoisoned,
}
