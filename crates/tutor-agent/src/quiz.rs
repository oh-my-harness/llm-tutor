use std::sync::Arc;

use llm_adapter::Provider;
use llm_adapter::types::{ChatRequest, Message, RequestContent, ResponseContent, ResponseFormat};
use serde::{Deserialize, Serialize};

use crate::error::{Result, TutorError};
use crate::llm_provider::LlmConfig;

#[derive(Debug, Clone)]
pub struct QuizGenerationConfig {
    pub topic: Option<String>,
    pub difficulty: String,
    pub question_count: usize,
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
    #[serde(default)]
    pub citation_indices: Vec<usize>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GeneratedQuiz {
    questions: Vec<GeneratedQuizQuestion>,
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
    if chunks.is_empty() {
        return Err(TutorError::Internal(
            "quiz generation has no source chunks".into(),
        ));
    }

    let prompt = generation_prompt(config, chunks);
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
    "You generate grounded tutor quiz questions. Return only valid JSON. Every question must be answerable from the supplied source chunks.".into()
}

fn generation_prompt(config: &QuizGenerationConfig, chunks: &[QuizSourceChunk]) -> String {
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

    format!(
        "Create {count} single-choice questions.\nTopic: {topic}\nDifficulty: {difficulty}\n\nSources:\n{sources}\n\nReturn JSON exactly like:\n{{\"questions\":[{{\"stem\":\"...\",\"options\":[\"...\",\"...\",\"...\",\"...\"],\"correct_option_index\":0,\"explanation\":\"...\",\"citation_indices\":[0],\"tags\":[\"...\"]}}]}}",
        count = config.question_count.clamp(1, 10),
        difficulty = config.difficulty,
    )
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
                        "citation_indices": {
                            "type": "array",
                            "items": { "type": "integer", "minimum": 0 }
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["stem", "options", "correct_option_index", "explanation", "citation_indices", "tags"]
                }
            }
        },
        "required": ["questions"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_adapter::LlmError;
    use llm_adapter::provider::ProviderCapabilities;
    use llm_adapter::stream_handle::StreamHandle;
    use llm_adapter::types::{ChatResponse, StopReason, Usage};

    struct FakeQuizProvider;

    #[async_trait::async_trait]
    impl Provider for FakeQuizProvider {
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::new(false, false, true)
        }

        async fn chat(&self, _req: &ChatRequest) -> std::result::Result<ChatResponse, LlmError> {
            Ok(ChatResponse {
                id: "fake".into(),
                model: "fake-model".into(),
                content: vec![ResponseContent::Text(r#"{"questions":[{"stem":"What does OPC do?","options":["Corrects mask patterns","Ignores masks"],"correct_option_index":0,"explanation":"The source says OPC corrects mask patterns.","citation_indices":[0],"tags":["OPC"]}]}"#.into())],
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
            Arc::new(FakeQuizProvider),
            "fake-model",
            &QuizGenerationConfig {
                topic: Some("OPC".into()),
                difficulty: "medium".into(),
                question_count: 1,
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

    #[test]
    fn parses_and_validates_generated_quiz_json() {
        let text = r#"{"questions":[{"stem":"What does OPC do?","options":["Corrects mask patterns","Ignores masks"],"correct_option_index":0,"explanation":"The source says OPC corrects mask patterns.","citation_indices":[0],"tags":["OPC"]}]}"#;

        let questions = parse_generated_quiz(text, 2, 1).unwrap();

        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].correct_option_index, 0);
        assert_eq!(questions[0].citation_indices, vec![0]);
    }

    #[test]
    fn rejects_out_of_range_citations() {
        let text = r#"{"questions":[{"stem":"Q?","options":["A","B"],"correct_option_index":0,"explanation":"Because.","citation_indices":[2],"tags":[]}]}"#;

        let err = parse_generated_quiz(text, 1, 1).unwrap_err().to_string();

        assert!(err.contains("citation index"));
    }
}
