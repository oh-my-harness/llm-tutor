use std::sync::Arc;

use llm_harness_runtime::audit::{AuditEntry, AuditEventType, AuditSink};
use llm_harness_runtime::budget::BudgetControlAdapter;
use llm_harness_runtime::human_approval::HumanApprovalWrapper;
use uuid::Uuid;

/// Session-wide governance configuration shared across all harnesses.
#[derive(Clone)]
pub struct GovernanceConfig {
    /// Shared budget adapter — tracks cumulative cost across all harness sessions.
    pub budget: Arc<BudgetControlAdapter>,
    /// Optional audit sink for writing structured learning-trail events.
    pub audit: Option<Arc<dyn AuditSink>>,
    /// Optional human approval gate (wraps `BeforeToolCallHook`).
    pub approval: Option<Arc<HumanApprovalWrapper>>,
    /// When true, `code_exec` calls require human approval.
    pub require_code_exec_approval: bool,
}

impl GovernanceConfig {
    pub fn new(
        budget: Arc<BudgetControlAdapter>,
        audit: Option<Arc<dyn AuditSink>>,
        require_code_exec_approval: bool,
    ) -> Self {
        Self {
            budget,
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
    use llm_harness_runtime::budget::BudgetControlAdapter;
    use llm_harness_runtime::cost::{PricingProvider, TokenPrice};
    use std::sync::Arc;

    struct NoPricing;
    impl PricingProvider for NoPricing {
        fn price_for(&self, _model: &str, _provider: &str) -> Option<TokenPrice> {
            Some(TokenPrice {
                input_per_mtok: 0.0,
                output_per_mtok: 0.0,
                cache_read_per_mtok: 0.0,
                cache_write_per_mtok: 0.0,
            })
        }
    }

    #[test]
    fn governance_config_builds_without_approval() {
        let budget = Arc::new(BudgetControlAdapter::new(Arc::new(NoPricing), 2.0, None));
        let cfg = GovernanceConfig::new(budget, None, false);
        assert!(!cfg.require_code_exec_approval);
    }
}
