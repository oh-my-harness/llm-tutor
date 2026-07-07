use std::sync::Arc;

use llm_adapter::Provider;
use llm_adapter::types::{ChatRequest, Message, RequestContent, ResponseContent, ResponseFormat};
use serde::{Deserialize, Serialize};

use crate::error::{Result, TutorError};
use crate::llm_provider::LlmConfig;
use crate::runtime_workflow::validate_quiz_generation_workflow;

#[derive(Debug, Clone)]
pub struct QuizGenerationConfig {
    pub topic: Option<String>,
    pub difficulty: String,
    pub question_count: usize,
    pub memory_markdown: Option<String>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Deserialize)]
struct QuizVerification {
    verdict: String,
    #[serde(default)]
    issues: Vec<QuizVerificationIssue>,
}

#[derive(Debug, Deserialize)]
struct QuizVerificationIssue {
    question_index: Option<usize>,
    severity: Option<String>,
    message: String,
}

pub async fn generate_quiz_questions(
    llm: &LlmConfig,
    config: &QuizGenerationConfig,
    chunks: &[QuizSourceChunk],
) -> Result<Vec<GeneratedQuizQuestion>> {
    generate_quiz_questions_with_client(llm.build_client(), &llm.model, config, chunks).await
}

pub async fn generate_quiz_questions_with_client(
    client: Arc<dyn Provider>,
    model: &str,
    config: &QuizGenerationConfig,
    chunks: &[QuizSourceChunk],
) -> Result<Vec<GeneratedQuizQuestion>> {
    validate_quiz_generation_workflow()?;

    if chunks.is_empty() {
        return Err(TutorError::Internal(
            "quiz generation has no source chunks".into(),
        ));
    }

    let mut questions = generate_quiz_attempt(client.clone(), model, config, chunks, None).await?;
    repair_supporting_quotes_against_chunks(&mut questions, chunks);
    match verify_quiz_questions_with_client(client.clone(), model, &questions, chunks).await {
        Ok(()) => Ok(questions),
        Err(first_error) => {
            let repair_feedback = first_error.to_string();
            let mut repaired_questions =
                generate_quiz_attempt(client.clone(), model, config, chunks, Some(&repair_feedback))
                    .await?;
            repair_supporting_quotes_against_chunks(&mut repaired_questions, chunks);
            verify_quiz_questions_with_client(client, model, &repaired_questions, chunks)
                .await
                .map_err(|second_error| {
                    TutorError::Internal(format!(
                        "quiz verifier rejected generated questions after repair attempt: first attempt: {first_error}; repair attempt: {second_error}"
                    ))
                })?;
            Ok(repaired_questions)
        }
    }
}

async fn generate_quiz_attempt(
    client: Arc<dyn Provider>,
    model: &str,
    config: &QuizGenerationConfig,
    chunks: &[QuizSourceChunk],
    repair_feedback: Option<&str>,
) -> Result<Vec<GeneratedQuizQuestion>> {
    let prompt = generation_prompt(config, chunks, repair_feedback);
    let mut builder = ChatRequest::builder(model, 2048)
        .message(Message::System(system_prompt()))
        .message(Message::User(vec![RequestContent::Text(prompt)]))
        .temperature(0.2);

    if client.capabilities().supports_json_schema() {
        builder = builder.response_format(ResponseFormat::JsonSchema {
            name: "quiz_questions".into(),
            schema: quiz_schema(),
            strict: Some(true),
        });
    } else {
        builder = builder.response_format(ResponseFormat::JsonObject);
    }

    let response = client
        .chat(&builder.build())
        .await
        .map_err(|err| TutorError::Internal(format!("quiz LLM generation failed: {err}")))?;
    let text = response_text(&response.content);
    parse_generated_quiz(&text, config.question_count, chunks.len())
}

async fn verify_quiz_questions_with_client(
    client: Arc<dyn Provider>,
    model: &str,
    questions: &[GeneratedQuizQuestion],
    chunks: &[QuizSourceChunk],
) -> Result<()> {
    let prompt = verification_prompt(questions, chunks)?;
    let mut builder = ChatRequest::builder(model, 1024)
        .message(Message::System(verification_system_prompt()))
        .message(Message::User(vec![RequestContent::Text(prompt)]))
        .temperature(0.0);

    if client.capabilities().supports_json_schema() {
        builder = builder.response_format(ResponseFormat::JsonSchema {
            name: "quiz_verification".into(),
            schema: verification_schema(),
            strict: Some(true),
        });
    } else {
        builder = builder.response_format(ResponseFormat::JsonObject);
    }

    let response = client
        .chat(&builder.build())
        .await
        .map_err(|err| TutorError::Internal(format!("quiz verifier failed: {err}")))?;
    parse_quiz_verification(&response_text(&response.content))
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

fn system_prompt() -> String {
    "You generate grounded tutor quiz questions. Return only valid JSON. Every question must be answerable from the supplied source chunks. The correct answer, explanation, citation_indices, and supporting_quote must all agree with each other.".into()
}

fn verification_system_prompt() -> String {
    "You are a strict quiz verifier. Return only JSON. Reject any question whose correct answer, explanation, or citation is not directly supported by the cited source chunks.".into()
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

fn verification_prompt(
    questions: &[GeneratedQuizQuestion],
    chunks: &[QuizSourceChunk],
) -> Result<String> {
    let payload = serde_json::json!({
        "questions": questions,
        "sources": chunks.iter().enumerate().map(|(index, chunk)| serde_json::json!({
            "index": index,
            "source": chunk.source,
            "text": chunk.text,
        })).collect::<Vec<_>>(),
        "rules": [
            "The correct option must be directly supported by the cited source chunks.",
            "The explanation must not contradict the correct option.",
            "supporting_quote must support the correct option.",
            "citation_indices must not cite merely topical but unsupported chunks.",
            "Return fail for any hallucination, answer/explanation mismatch, or wrong citation."
        ]
    });
    serde_json::to_string_pretty(&payload)
        .map_err(|err| TutorError::Internal(format!("failed to build quiz verifier prompt: {err}")))
}

fn parse_quiz_verification(text: &str) -> Result<()> {
    let json_text = extract_json_object(text)
        .ok_or_else(|| TutorError::Internal("quiz verifier output did not contain JSON".into()))?;
    let parsed: QuizVerification = serde_json::from_str(json_text)
        .map_err(|err| TutorError::Internal(format!("invalid quiz verifier JSON: {err}")))?;
    if parsed.verdict.trim().eq_ignore_ascii_case("pass") {
        return Ok(());
    }

    let issues = parsed
        .issues
        .into_iter()
        .map(|issue| {
            let location = issue
                .question_index
                .map(|index| format!("question {index}"))
                .unwrap_or_else(|| "quiz".into());
            let severity = issue.severity.unwrap_or_else(|| "issue".into());
            format!("{location} {severity}: {}", issue.message)
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(TutorError::Internal(format!(
        "quiz verifier rejected generated questions{}",
        if issues.is_empty() {
            String::new()
        } else {
            format!(": {issues}")
        }
    )))
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

fn response_text(content: &[ResponseContent]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ResponseContent::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start <= end).then_some(&text[start..=end])
}

fn quiz_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "questions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "stem": { "type": "string" },
                        "options": {
                            "type": "array",
                            "minItems": 2,
                            "items": { "type": "string" }
                        },
                        "correct_option_index": { "type": "integer", "minimum": 0 },
                        "explanation": { "type": "string" },
                        "supporting_quote": { "type": "string" },
                        "citation_indices": {
                            "type": "array",
                            "minItems": 1,
                            "items": { "type": "integer", "minimum": 0 }
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["stem", "options", "correct_option_index", "explanation", "supporting_quote", "citation_indices", "tags"]
                }
            }
        },
        "required": ["questions"]
    })
}

fn verification_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "verdict": { "type": "string", "enum": ["pass", "fail"] },
            "issues": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "question_index": { "type": ["integer", "null"], "minimum": 0 },
                        "severity": { "type": ["string", "null"] },
                        "message": { "type": "string" }
                    },
                    "required": ["question_index", "severity", "message"]
                }
            }
        },
        "required": ["verdict", "issues"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use llm_adapter::LlmError;
    use llm_adapter::provider::ProviderCapabilities;
    use llm_adapter::stream_handle::StreamHandle;
    use llm_adapter::types::{ChatResponse, StopReason, Usage};

    struct FakeQuizProvider {
        calls: AtomicUsize,
    }

    impl FakeQuizProvider {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl Provider for FakeQuizProvider {
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::new(false, false, true)
        }

        async fn chat(&self, _req: &ChatRequest) -> std::result::Result<ChatResponse, LlmError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let content = if call == 0 {
                r#"{"questions":[{"stem":"What does OPC do?","options":["Corrects mask patterns","Ignores masks"],"correct_option_index":0,"explanation":"The source says OPC corrects mask patterns, so ignoring masks is not supported.","supporting_quote":"OPC corrects lithography mask patterns","citation_indices":[0],"tags":["OPC"]}]}"#
            } else {
                r#"{"verdict":"pass","issues":[]}"#
            };
            Ok(ChatResponse {
                id: "fake".into(),
                model: "fake-model".into(),
                content: vec![ResponseContent::Text(content.into())],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            })
        }

        async fn chat_stream(
            &self,
            _req: &ChatRequest,
        ) -> std::result::Result<StreamHandle, LlmError> {
            unimplemented!("quiz generation uses non-streaming chat in this test")
        }
    }

    #[tokio::test]
    async fn generates_questions_with_fake_provider() {
        let questions = generate_quiz_questions_with_client(
            Arc::new(FakeQuizProvider::new()),
            "fake-model",
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
        )
        .await
        .unwrap();

        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].options[0], "Corrects mask patterns");
    }

    struct RepairingQuizProvider {
        calls: AtomicUsize,
    }

    impl RepairingQuizProvider {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl Provider for RepairingQuizProvider {
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::new(false, false, true)
        }

        async fn chat(&self, _req: &ChatRequest) -> std::result::Result<ChatResponse, LlmError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let content = match call {
                0 => r#"{"questions":[{"stem":"Wrong draft","options":["Corrects mask patterns","Ignores masks"],"correct_option_index":1,"explanation":"The source supports correcting mask patterns.","supporting_quote":"OPC corrects lithography mask patterns","citation_indices":[0],"tags":["OPC"]}]}"#,
                1 => r#"{"verdict":"fail","issues":[{"question_index":0,"severity":"high","message":"correct answer contradicts explanation and source"}]}"#,
                2 => r#"{"questions":[{"stem":"Repaired draft","options":["Corrects mask patterns","Ignores masks"],"correct_option_index":0,"explanation":"The source supports correcting mask patterns; ignoring masks is not supported.","supporting_quote":"OPC corrects lithography mask patterns","citation_indices":[0],"tags":["OPC"]}]}"#,
                _ => r#"{"verdict":"pass","issues":[]}"#,
            };
            Ok(ChatResponse {
                id: "fake".into(),
                model: "fake-model".into(),
                content: vec![ResponseContent::Text(content.into())],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            })
        }

        async fn chat_stream(
            &self,
            _req: &ChatRequest,
        ) -> std::result::Result<StreamHandle, LlmError> {
            unimplemented!("quiz generation uses non-streaming chat in this test")
        }
    }

    #[tokio::test]
    async fn repairs_once_when_verifier_rejects_first_draft() {
        let provider = Arc::new(RepairingQuizProvider::new());
        let questions = generate_quiz_questions_with_client(
            provider.clone(),
            "fake-model",
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
        )
        .await
        .unwrap();

        assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
        assert_eq!(questions[0].stem, "Repaired draft");
        assert_eq!(questions[0].correct_option_index, 0);
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
    fn verifier_rejects_answer_explanation_contradictions() {
        let err = parse_quiz_verification(
            r#"{"verdict":"fail","issues":[{"question_index":0,"severity":"high","message":"correct answer contradicts explanation"}]}"#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("rejected"));
        assert!(err.contains("contradicts explanation"));
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
