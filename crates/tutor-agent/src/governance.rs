use std::sync::Arc;

use llm_harness_runtime::control::human_approval::HumanApprovalWrapper;
use llm_harness_runtime::observability::audit::{AuditEntry, AuditEventType, AuditSink};
use uuid::Uuid;

/// Session-wide governance configuration shared across all harnesses.
#[derive(Clone)]
pub struct GovernanceConfig {
    /// Session budget limit in USD. The product stores the limit here while
    /// runtime budget-policy APIs are still being hardened.
    pub budget_limit_usd: f64,
    /// Optional audit sink for writing structured learning-trail events.
    pub audit: Option<Arc<dyn AuditSink>>,
    /// Optional human approval gate (wraps `BeforeToolCallHook`).
    pub approval: Option<Arc<HumanApprovalWrapper>>,
    /// When true, `code_exec` calls require human approval.
    pub require_code_exec_approval: bool,
}

impl GovernanceConfig {
    pub fn new(
        budget_limit_usd: f64,
        audit: Option<Arc<dyn AuditSink>>,
        require_code_exec_approval: bool,
    ) -> Self {
        Self {
            budget_limit_usd,
            audit,
            approval: None,
            require_code_exec_approval,
        }
    }

    pub fn with_approval(mut self, approval: Arc<HumanApprovalWrapper>) -> Self {
        self.approval = Some(approval);
        self
    }
}

/// Helper to create an audit entry with reasonable defaults for v0.1.
/// The `hash` and `prev_hash` fields are overwritten by `JsonlAuditSink::record`.
pub fn make_audit_entry(event_type: AuditEventType, payload: serde_json::Value) -> AuditEntry {
    AuditEntry {
        timestamp: chrono::Utc::now(),
        trace_id: String::new(),
        session_id: llm_harness_types::EntryId(Uuid::new_v4()),
        task_id: None,
        principal: "system".into(),
        event_type,
        payload,
        decision: None,
        cost_delta: None,
        prev_hash: None,
        hash: String::new(),
    }
}

/// Record an audit event if the audit sink is configured.
pub async fn record_audit(
    audit: &Option<Arc<dyn AuditSink>>,
    event_type: AuditEventType,
    payload: serde_json::Value,
) {
    if let Some(sink) = audit {
        let entry = make_audit_entry(event_type, payload);
        let _ = sink.record(entry).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn governance_config_builds_without_approval() {
        let cfg = GovernanceConfig::new(2.0, None, false);
        assert!(!cfg.require_code_exec_approval);
        assert_eq!(cfg.budget_limit_usd, 2.0);
    }
}
