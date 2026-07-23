use std::sync::Arc;

use futures::future::BoxFuture;
use llm_harness_runtime::control::cost::CostAggregate;
use llm_harness_runtime::workflow::engine::{WorkflowEngine, WorkflowEngineConfig};
use llm_harness_runtime::workflow::executor::{ExecutorCtx, StepExecutor};
use llm_harness_runtime::workflow::judge::{StepCtx, StepTransitionJudge};
use llm_harness_runtime::workflow::model::{StepResult, StructuredStatus, Transition};
use serde::{Deserialize, Serialize};

use crate::error::{Result, TutorError};
use crate::runtime_workflow::{quiz_generation_workflow, validate_quiz_generation_workflow};

#[derive(Debug, Clone)]
pub struct QuizGenerationConfig {
    pub topic: Option<String>,
    pub difficulty: String,
    pub question_count: usize,
    pub memory_markdown: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizSourceChunk {
    pub source: String,
    pub text: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedQuizQuestion {
    pub stem: String,
    pub options: Vec<String>,
    pub correct_option_index: usize,
    pub explanation: String,
    pub supporting_quote: String,
    #[serde(default)]
    pub citation_indices: Vec<usize>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GeneratedQuiz {
    questions: Vec<GeneratedQuizQuestion>,
}

#[derive(Debug, Clone)]
pub struct QuizWorkflowOutput {
    pub questions: Vec<GeneratedQuizQuestion>,
    pub cost: CostAggregate,
}

pub async fn generate_quiz_questions_with_workflow(
    config: &QuizGenerationConfig,
    chunks: &[QuizSourceChunk],
    engine_config: WorkflowEngineConfig,
) -> Result<QuizWorkflowOutput> {
    validate_quiz_generation_workflow()?;
    if chunks.is_empty() {
        return Err(TutorError::Internal(
            "quiz generation has no source chunks".into(),
        ));
    }

    let workflow = quiz_generation_workflow();
    let engine = WorkflowEngine::new(workflow, engine_config, Arc::new(QuizWorkflowJudge))
        .map_err(|err| TutorError::Internal(format!("quiz workflow initialization failed: {err}")))?
        .with_executor(
            "tutor.quiz.collect_sources",
            Arc::new(CollectQuizSourcesExecutor {
                config: config.clone(),
                chunks: chunks.to_vec(),
            }),
        )
        .with_executor(
            "tutor.quiz.publish_questions",
            Arc::new(PublishQuizQuestionsExecutor {
                expected_count: config.question_count,
                chunks: chunks.to_vec(),
            }),
        )
        .with_max_retries(1);

    let result = engine
        .run()
        .await
        .map_err(|err| TutorError::Internal(format!("quiz workflow failed: {err}")))?;

    let publish = engine
        .step_history()
        .await
        .into_iter()
        .rev()
        .find(|record| record.step_id == "publish_questions")
        .and_then(|record| record.result)
        .and_then(|result| result.structured)
        .ok_or_else(|| TutorError::Internal("quiz workflow did not publish questions".into()))?;

    let questions = publish.get("questions").cloned().ok_or_else(|| {
        TutorError::Internal("quiz workflow publish result is missing questions".into())
    })?;
    let questions = serde_json::from_value(questions)
        .map_err(|err| TutorError::Internal(format!("invalid quiz workflow questions: {err}")))?;
    Ok(QuizWorkflowOutput {
        questions,
        cost: result.cost,
    })
}

struct QuizWorkflowJudge;

impl StepTransitionJudge for QuizWorkflowJudge {
    fn decide<'a>(&'a self, ctx: &StepCtx<'a>) -> BoxFuture<'a, Transition> {
        let current_step = ctx.current_step.id().clone();
        let structured = ctx.last_result.structured.clone();
        let generate_attempts = ctx
            .step_history
            .iter()
            .filter(|record| record.step_id == "generate_questions")
            .count();
        Box::pin(async move {
            match current_step.as_str() {
                "publish_questions" => Transition::Abort {
                    reason: "quiz generated".into(),
                },
                "collect_sources" => Transition::To("generate_questions".into()),
                "generate_questions" => Transition::To("verify_questions".into()),
                "verify_questions" => {
                    let verdict = structured
                        .as_ref()
                        .and_then(|value| value.get("verdict"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    if verdict.eq_ignore_ascii_case("pass") {
                        return Transition::To("publish_questions".into());
                    }
                    if generate_attempts >= 2 {
                        return Transition::Fail {
                            reason: format!(
                                "quiz verifier rejected generated questions after repair attempt: {}",
                                structured
                                    .as_ref()
                                    .and_then(|value| serde_json::to_string(value).ok())
                                    .unwrap_or_else(|| "missing structured verifier output".into())
                            ),
                        };
                    }
                    Transition::To("generate_questions".into())
                }
                _ => Transition::Fail {
                    reason: format!("quiz workflow has no transition from {current_step}"),
                },
            }
        })
    }
}

struct CollectQuizSourcesExecutor {
    config: QuizGenerationConfig,
    chunks: Vec<QuizSourceChunk>,
}

impl StepExecutor for CollectQuizSourcesExecutor {
    fn execute<'a>(
        &'a self,
        ctx: &'a ExecutorCtx<'a>,
    ) -> BoxFuture<'a, anyhow::Result<StepResult>> {
        Box::pin(async move {
            let source_count = self.chunks.len();
            {
                let mut context = ctx.context.lock().await;
                context.variables.insert(
                    "quiz_generation_prompt".into(),
                    serde_json::json!(generation_prompt(&self.config, &self.chunks, None)),
                );
                context
                    .variables
                    .insert("quiz_sources".into(), serde_json::to_value(&self.chunks)?);
                context.variables.insert(
                    "quiz_expected_count".into(),
                    serde_json::json!(self.config.question_count.clamp(1, 10)),
                );
            }
            Ok(workflow_step_result(
                format!("collected {source_count} source chunks"),
                serde_json::json!({ "source_count": source_count }),
            ))
        })
    }
}

struct PublishQuizQuestionsExecutor {
    expected_count: usize,
    chunks: Vec<QuizSourceChunk>,
}

impl StepExecutor for PublishQuizQuestionsExecutor {
    fn execute<'a>(
        &'a self,
        ctx: &'a ExecutorCtx<'a>,
    ) -> BoxFuture<'a, anyhow::Result<StepResult>> {
        Box::pin(async move {
            let generated = ctx
                .step_history
                .iter()
                .rev()
                .find(|record| record.step_id == "generate_questions")
                .and_then(|record| record.result.as_ref())
                .and_then(|result| result.structured.as_ref())
                .ok_or_else(|| anyhow::anyhow!("quiz publish has no generated questions"))?;
            let questions_value = generated
                .get("questions")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("quiz generation result missing questions"))?;
            let parsed = GeneratedQuiz {
                questions: serde_json::from_value(questions_value)?,
            };
            let mut questions =
                validate_questions(parsed.questions, self.expected_count, self.chunks.len())?;
            repair_supporting_quotes_against_chunks(&mut questions, &self.chunks);
            Ok(workflow_step_result(
                format!("published {} quiz questions", questions.len()),
                serde_json::json!({ "questions": questions }),
            ))
        })
    }
}

fn workflow_step_result(output: String, structured: serde_json::Value) -> StepResult {
    StepResult {
        output,
        structured: Some(structured),
        structured_status: StructuredStatus::NotRequired,
        tool_calls_count: 0,
        session_id: String::new(),
        cost: CostAggregate::default(),
        started_at: None,
        ended_at: None,
    }
}

pub fn parse_generated_quiz(
    text: &str,
    expected_count: usize,
    source_count: usize,
) -> Result<Vec<GeneratedQuizQuestion>> {
    let json_text = extract_json_object(text)
        .ok_or_else(|| TutorError::Internal("quiz LLM output did not contain JSON".into()))?;
    let parsed: GeneratedQuiz = serde_json::from_str(json_text)
        .map_err(|err| TutorError::Internal(format!("invalid quiz JSON: {err}")))?;
    validate_questions(parsed.questions, expected_count, source_count)
}

fn validate_questions(
    questions: Vec<GeneratedQuizQuestion>,
    expected_count: usize,
    source_count: usize,
) -> Result<Vec<GeneratedQuizQuestion>> {
    if questions.is_empty() {
        return Err(TutorError::Internal(
            "quiz LLM output has no questions".into(),
        ));
    }
    if questions.len() > expected_count.clamp(1, 10) {
        return Err(TutorError::Internal(
            "quiz LLM output has too many questions".into(),
        ));
    }
    for question in &questions {
        if question.stem.trim().is_empty() {
            return Err(TutorError::Internal("quiz question stem is empty".into()));
        }
        if question.options.len() < 2 {
            return Err(TutorError::Internal(
                "quiz question has fewer than two options".into(),
            ));
        }
        if question.correct_option_index >= question.options.len() {
            return Err(TutorError::Internal(
                "quiz question correct option index is out of range".into(),
            ));
        }
        if question.explanation.trim().is_empty() {
            return Err(TutorError::Internal(
                "quiz question explanation is empty".into(),
            ));
        }
        if question.supporting_quote.trim().is_empty() {
            return Err(TutorError::Internal(
                "quiz question supporting quote is empty".into(),
            ));
        }
        let mut normalized_options = std::collections::HashSet::new();
        for option in &question.options {
            let normalized = normalize_text(option);
            if normalized.is_empty() {
                return Err(TutorError::Internal("quiz option is empty".into()));
            }
            if !normalized_options.insert(normalized) {
                return Err(TutorError::Internal(
                    "quiz question has duplicate options".into(),
                ));
            }
        }
        if question.citation_indices.is_empty() {
            return Err(TutorError::Internal(
                "quiz question has no citations".into(),
            ));
        }
        for index in &question.citation_indices {
            if *index >= source_count {
                return Err(TutorError::Internal(
                    "quiz citation index is out of range".into(),
                ));
            }
        }
    }
    Ok(questions)
}

fn generation_prompt(
    config: &QuizGenerationConfig,
    chunks: &[QuizSourceChunk],
    repair_feedback: Option<&str>,
) -> String {
    let topic = config
        .topic
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("the selected knowledge base");
    let sources = chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            format!(
                "[{index}] source: {}\nscore: {:?}\ntext: {}",
                chunk.source, chunk.score, chunk.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let memory = config
        .memory_markdown
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            format!("\nLearner memory for personalization only, not factual support:\n{value}\n")
        })
        .unwrap_or_default();
    let repair = repair_feedback
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            format!(
                "\nPrevious draft was rejected by the verifier. Repair these issues and return a fresh complete JSON object:\n{value}\n"
            )
        })
        .unwrap_or_default();

    format!(
        "Create {count} single-choice questions.\nTopic: {topic}\nDifficulty: {difficulty}\n{memory}{repair}\nRules:\n- Use learner memory only to choose focus, difficulty, tags, and explanation style.\n- Use only facts that are directly supported by the supplied sources.\n- The option at correct_option_index must be the only best answer.\n- The explanation must explicitly explain why the correct option is correct and why the key distractor is not supported.\n- citation_indices must point only to source chunks that support the correct answer.\n- supporting_quote must be an exact short quote copied from one cited source chunk and must support the correct answer.\n- Do not cite learner memory.\n- Do not cite a source chunk merely because it is topically related.\n\nSources:\n{sources}\n\nReturn JSON exactly like:\n{{\"questions\":[{{\"stem\":\"...\",\"options\":[\"...\",\"...\",\"...\",\"...\"],\"correct_option_index\":0,\"explanation\":\"...\",\"supporting_quote\":\"exact quote from cited source\",\"citation_indices\":[0],\"tags\":[\"...\"]}}]}}",
        count = config.question_count.clamp(1, 10),
        difficulty = config.difficulty,
        repair = repair,
    )
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn repair_supporting_quotes_against_chunks(
    questions: &mut [GeneratedQuizQuestion],
    chunks: &[QuizSourceChunk],
) {
    for question in questions {
        if quote_found_in_cited_chunks(question, chunks) {
            continue;
        }
        let Some(source) = question
            .citation_indices
            .iter()
            .find_map(|index| chunks.get(*index))
        else {
            continue;
        };
        question.supporting_quote = source_quote_prefix(&source.text, 240);
    }
}

fn quote_found_in_cited_chunks(
    question: &GeneratedQuizQuestion,
    chunks: &[QuizSourceChunk],
) -> bool {
    let quote = normalize_text(&question.supporting_quote);
    if quote.is_empty() {
        return false;
    }
    question.citation_indices.iter().any(|index| {
        chunks
            .get(*index)
            .map(|chunk| normalize_text(&chunk.text).contains(&quote))
            .unwrap_or(false)
    })
}

fn source_quote_prefix(source_text: &str, max_chars: usize) -> String {
    normalize_text(source_text)
        .chars()
        .take(max_chars)
        .collect::<String>()
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start <= end).then_some(&text[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    use llm_harness_loop::test_utils::{MockLlmClient, MockResponse, NoOpEnv};
    use llm_harness_types::ExecutionEnv;

    use crate::runtime_engine::build_workflow_engine_config;

    #[tokio::test]
    async fn runtime_workflow_repairs_and_publishes_quiz() {
        let dir = tempfile::TempDir::new().unwrap();
        let client = Arc::new(MockLlmClient::new(vec![
            MockResponse::text(
                r#"{"questions":[{"stem":"Wrong draft","options":["Corrects mask patterns","Ignores masks"],"correct_option_index":1,"explanation":"The source supports correcting mask patterns.","supporting_quote":"OPC corrects lithography mask patterns","citation_indices":[0],"tags":["OPC"]}]}"#,
            ),
            MockResponse::text(
                r#"{"verdict":"fail","action":"repair","issues":["correct answer contradicts explanation and source"]}"#,
            ),
            MockResponse::text(
                r#"{"questions":[{"stem":"Repaired draft","options":["Corrects mask patterns","Ignores masks"],"correct_option_index":0,"explanation":"The source supports correcting mask patterns; ignoring masks is not supported.","supporting_quote":"OPC corrects lithography mask patterns","citation_indices":[0],"tags":["OPC"]}]}"#,
            ),
            MockResponse::text(r#"{"verdict":"pass","issues":[]}"#),
        ]));
        let engine_config = build_workflow_engine_config(
            client.clone(),
            "fake-model",
            Arc::new(NoOpEnv) as Arc<dyn ExecutionEnv>,
            dir.path().join("quiz-workflow-sessions"),
        );

        let output = generate_quiz_questions_with_workflow(
            &QuizGenerationConfig {
                topic: Some("OPC".into()),
                difficulty: "medium".into(),
                question_count: 1,
                memory_markdown: None,
            },
            &[QuizSourceChunk {
                source: "source.md".into(),
                text: "OPC corrects lithography mask patterns before wafer exposure.".into(),
                score: Some(0.9),
            }],
            engine_config,
        )
        .await
        .unwrap();

        assert_eq!(
            client.call_count.load(std::sync::atomic::Ordering::SeqCst),
            4
        );
        assert_eq!(output.questions[0].stem, "Repaired draft");
        assert_eq!(output.questions[0].correct_option_index, 0);
        assert_eq!(output.cost.total_input_tokens, 0);
    }

    #[test]
    fn parses_and_validates_generated_quiz_json() {
        let text = r#"{"questions":[{"stem":"What does OPC do?","options":["Corrects mask patterns","Ignores masks"],"correct_option_index":0,"explanation":"The source says OPC corrects mask patterns, so ignoring masks is not supported.","supporting_quote":"OPC corrects lithography mask patterns","citation_indices":[0],"tags":["OPC"]}]}"#;

        let questions = parse_generated_quiz(text, 2, 1).unwrap();

        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].correct_option_index, 0);
        assert_eq!(questions[0].citation_indices, vec![0]);
    }

    #[test]
    fn generation_prompt_uses_memory_only_for_personalization() {
        let prompt = generation_prompt(
            &QuizGenerationConfig {
                topic: Some("OPC".into()),
                difficulty: "medium".into(),
                question_count: 1,
                memory_markdown: Some("- Learner confuses OPC and photoresist.".into()),
            },
            &[QuizSourceChunk {
                source: "source.md".into(),
                text: "OPC corrects lithography mask patterns.".into(),
                score: None,
            }],
            None,
        );

        assert!(prompt.contains("Learner memory for personalization only"));
        assert!(prompt.contains("Do not cite learner memory"));
    }

    #[test]
    fn generation_prompt_can_include_repair_feedback() {
        let prompt = generation_prompt(
            &QuizGenerationConfig {
                topic: Some("OPC".into()),
                difficulty: "medium".into(),
                question_count: 1,
                memory_markdown: None,
            },
            &[QuizSourceChunk {
                source: "source.md".into(),
                text: "OPC corrects lithography mask patterns.".into(),
                score: None,
            }],
            Some("question 0 high: explanation contradicts answer"),
        );

        assert!(prompt.contains("Previous draft was rejected by the verifier"));
        assert!(prompt.contains("explanation contradicts answer"));
    }

    #[test]
    fn rejects_out_of_range_citations() {
        let text = r#"{"questions":[{"stem":"Q?","options":["A","B"],"correct_option_index":0,"explanation":"Because.","supporting_quote":"Because","citation_indices":[2],"tags":[]}]}"#;

        let err = parse_generated_quiz(text, 1, 1).unwrap_err().to_string();

        assert!(err.contains("citation index"));
    }

    #[test]
    fn rejects_questions_without_citations() {
        let text = r#"{"questions":[{"stem":"Q?","options":["A","B"],"correct_option_index":0,"explanation":"Because.","supporting_quote":"Because","citation_indices":[],"tags":[]}]}"#;

        let err = parse_generated_quiz(text, 1, 1).unwrap_err().to_string();

        assert!(err.contains("no citations"));
    }

    #[test]
    fn rejects_duplicate_options() {
        let text = r#"{"questions":[{"stem":"Q?","options":["Same"," Same "],"correct_option_index":0,"explanation":"Because.","supporting_quote":"Because","citation_indices":[0],"tags":[]}]}"#;

        let err = parse_generated_quiz(text, 1, 1).unwrap_err().to_string();

        assert!(err.contains("duplicate options"));
    }

    #[test]
    fn repairs_supporting_quote_not_found_in_cited_chunks() {
        let mut questions = vec![GeneratedQuizQuestion {
            stem: "Q?".into(),
            options: vec!["A".into(), "B".into()],
            correct_option_index: 0,
            explanation: "Because.".into(),
            supporting_quote: "not present".into(),
            citation_indices: vec![0],
            tags: vec![],
        }];
        let chunks = vec![QuizSourceChunk {
            source: "source.md".into(),
            text: "This chunk supports another fact.".into(),
            score: None,
        }];

        repair_supporting_quotes_against_chunks(&mut questions, &chunks);

        assert_eq!(
            questions[0].supporting_quote,
            "This chunk supports another fact."
        );
        assert!(quote_found_in_cited_chunks(&questions[0], &chunks));
    }
}
