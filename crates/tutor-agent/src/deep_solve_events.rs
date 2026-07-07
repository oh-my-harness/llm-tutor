use serde::Serialize;

use crate::event_sink::{SharedEventSink, emit_trace};

pub const CAPABILITY: &str = "deep_solve";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepSolveStage {
    Retrieve,
    Plan,
    Solve,
    Verify,
    Synthesize,
}

impl DeepSolveStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Retrieve => "retrieve",
            Self::Plan => "plan",
            Self::Solve => "solve",
            Self::Verify => "verify",
            Self::Synthesize => "synthesize",
        }
    }
}

pub async fn stage_start(sink: &Option<SharedEventSink>, stage: DeepSolveStage, title: &str) {
    emit_trace(
        sink,
        "deep_solve_stage_start",
        serde_json::json!({
            "capability": CAPABILITY,
            "stage": stage.as_str(),
            "title": title,
        }),
    )
    .await;
}

pub async fn stage_done(
    sink: &Option<SharedEventSink>,
    stage: DeepSolveStage,
    title: &str,
    summary: impl Into<String>,
) {
    emit_trace(
        sink,
        "deep_solve_stage_done",
        serde_json::json!({
            "capability": CAPABILITY,
            "stage": stage.as_str(),
            "title": title,
            "summary": summary.into(),
        }),
    )
    .await;
}

pub async fn step_start(sink: &Option<SharedEventSink>, step_id: &str, goal: &str) {
    emit_trace(
        sink,
        "deep_solve_step_start",
        serde_json::json!({
            "capability": CAPABILITY,
            "stage": DeepSolveStage::Solve.as_str(),
            "step_id": step_id,
            "title": goal,
        }),
    )
    .await;
}

pub async fn step_done(
    sink: &Option<SharedEventSink>,
    step_id: &str,
    goal: &str,
    summary: impl Into<String>,
) {
    emit_trace(
        sink,
        "deep_solve_step_done",
        serde_json::json!({
            "capability": CAPABILITY,
            "stage": DeepSolveStage::Solve.as_str(),
            "step_id": step_id,
            "title": goal,
            "summary": summary.into(),
        }),
    )
    .await;
}

pub async fn final_answer(sink: &Option<SharedEventSink>, text: &str) {
    emit_trace(
        sink,
        "deep_solve_final",
        serde_json::json!({
            "capability": CAPABILITY,
            "stage": DeepSolveStage::Synthesize.as_str(),
            "summary": text.chars().take(240).collect::<String>(),
        }),
    )
    .await;
}
