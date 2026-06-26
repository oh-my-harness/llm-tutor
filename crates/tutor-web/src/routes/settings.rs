use std::time::Duration;

use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::post};
use llm_adapter::types::{ChatRequest, Message, RequestContent};
use llm_adapter_embedding::EmbeddingProvider;
use llm_adapter_embedding::openai::OpenAIProvider;
use llm_adapter_embedding::types::EmbeddingRequest;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tutor_agent::llm_provider::{LlmConfig, LlmProviderKind};

pub fn settings_router() -> Router {
    Router::new()
        .route("/api/settings/test/llm", post(test_llm_config))
        .route("/api/settings/test/embedding", post(test_embedding_config))
}

#[derive(Debug, Deserialize)]
struct TestLlmRequest {
    provider: String,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
    chat_path: Option<String>,
    context_window_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct TestLlmResponse {
    ok: bool,
    provider: String,
    model: String,
    message: String,
    response_preview: String,
    confirmed_context_window_tokens: Option<u32>,
    context_window_source: Option<String>,
    context_window_detail: Option<String>,
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct TestEmbeddingRequest {
    provider: String,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
    embeddings_path: Option<String>,
    dimensions: Option<usize>,
    #[serde(default)]
    send_dimensions: bool,
}

#[derive(Debug, Serialize)]
struct TestEmbeddingResponse {
    ok: bool,
    provider: String,
    model: String,
    message: String,
    dimensions: usize,
    configured_dimensions: Option<usize>,
    dimensions_match: Option<bool>,
    input_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContextWindowDetection {
    context_window: Option<u32>,
    source: Option<&'static str>,
    detail: Option<String>,
}

impl ContextWindowDetection {
    fn message(&self) -> String {
        match (self.context_window, self.source) {
            (Some(tokens), Some("configured")) => {
                format!("连接成功，当前配置上下文窗口为 {tokens} tokens。")
            }
            (Some(tokens), Some("metadata")) => {
                format!("连接成功，从 provider metadata 检测到上下文窗口为 {tokens} tokens。")
            }
            (Some(tokens), Some("known_model")) => {
                format!("连接成功，按已知模型规则识别上下文窗口为 {tokens} tokens。")
            }
            (Some(tokens), Some("model_heuristic")) => {
                format!("连接成功，按模型名称估算上下文窗口约为 {tokens} tokens。")
            }
            (Some(tokens), _) => format!("连接成功，上下文窗口约为 {tokens} tokens。"),
            (None, _) => "连接成功，但无法自动确认上下文窗口。".into(),
        }
    }
}

async fn test_llm_config(Json(request): Json<TestLlmRequest>) -> impl IntoResponse {
    match run_llm_test(request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => json_error(StatusCode::BAD_REQUEST, error),
    }
}

async fn run_llm_test(request: TestLlmRequest) -> Result<TestLlmResponse, String> {
    let provider = parse_llm_provider(&request.provider)?;
    let model = non_empty(request.model, "请填写模型 ID")?;
    let api_key = non_empty_optional(request.api_key, "请填写 API Key")?;
    let base_url = optional_non_empty(request.base_url);
    let chat_path = optional_non_empty(request.chat_path);
    let configured_window = request.context_window_tokens.filter(|value| *value > 0);
    let llm = LlmConfig::from_parts(
        provider,
        model.clone(),
        api_key,
        base_url.clone(),
        chat_path,
        configured_window,
    );
    let client = llm.build_client();
    let req = ChatRequest::builder(&model, 16)
        .message(Message::System(
            "You are a connectivity test. Reply with OK.".into(),
        ))
        .message(Message::User(vec![RequestContent::Text(
            "Reply with OK only.".into(),
        )]))
        .temperature(0.0)
        .build();
    let response = client
        .chat(&req)
        .await
        .map_err(|err| format!("模型连接测试失败：{err}"))?;
    let detection = resolve_context_window(
        configured_window,
        &request.provider,
        &model,
        base_url.as_deref(),
        &llm.api_key,
    )
    .await;
    Ok(TestLlmResponse {
        ok: true,
        provider: request.provider,
        model,
        message: detection.message(),
        response_preview: response.text().chars().take(160).collect(),
        confirmed_context_window_tokens: detection.context_window,
        context_window_source: detection.source.map(str::to_string),
        context_window_detail: detection.detail,
        input_tokens: response.usage.input_tokens,
        output_tokens: response.usage.output_tokens,
    })
}

async fn test_embedding_config(Json(request): Json<TestEmbeddingRequest>) -> impl IntoResponse {
    match run_embedding_test(request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => json_error(StatusCode::BAD_REQUEST, error),
    }
}

async fn run_embedding_test(
    request: TestEmbeddingRequest,
) -> Result<TestEmbeddingResponse, String> {
    if !request.provider.eq_ignore_ascii_case("openai") {
        return Err("当前仅支持 OpenAI-compatible embedding 接口测试".into());
    }
    let model = non_empty(request.model, "请填写嵌入模型 ID")?;
    let api_key = non_empty_optional(request.api_key, "请填写 API Key")?;
    let mut builder = OpenAIProvider::builder(api_key);
    if let Some(base_url) = optional_non_empty(request.base_url) {
        builder = builder.base_url(base_url);
    }
    if let Some(path) = optional_non_empty(request.embeddings_path) {
        builder = builder.embeddings_path(path);
    }
    let provider = builder.build();
    let mut req = EmbeddingRequest::builder(model.clone()).input("llm-tutor embedding test");
    if request.send_dimensions {
        if let Some(dimensions) = request.dimensions.filter(|value| *value > 0) {
            req = req.dimensions(dimensions);
        }
    }
    let req = req.build();
    let response = provider
        .embed(&req)
        .await
        .map_err(|err| format!("嵌入模型连接测试失败：{err}"))?;
    let dimensions = response
        .dimensions()
        .ok_or_else(|| "嵌入模型测试没有返回向量".to_string())?;
    let configured_dimensions = request.dimensions.filter(|value| *value > 0);
    let dimensions_match = configured_dimensions.map(|configured| configured == dimensions);
    let usage = response.usage;
    Ok(TestEmbeddingResponse {
        ok: true,
        provider: request.provider,
        model,
        message: match dimensions_match {
            Some(true) => format!("连接成功，向量维度为 {dimensions}。"),
            Some(false) => format!("连接成功，实际向量维度为 {dimensions}，已用实际值更新配置。"),
            None => format!("连接成功，已确认向量维度为 {dimensions}。"),
        },
        dimensions,
        configured_dimensions,
        dimensions_match,
        input_tokens: usage.as_ref().map(|item| item.input_tokens),
        total_tokens: usage.as_ref().map(|item| item.total_tokens),
    })
}

fn parse_llm_provider(provider: &str) -> Result<LlmProviderKind, String> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "anthropic" => Ok(LlmProviderKind::Anthropic),
        "deepseek" => Ok(LlmProviderKind::DeepSeek),
        "openai" | "openai-compatible" => Ok(LlmProviderKind::OpenAI),
        _ => Err("不支持的 LLM 接口模式".into()),
    }
}

async fn resolve_context_window(
    configured: Option<u32>,
    provider: &str,
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
) -> ContextWindowDetection {
    if let Some(value) = configured {
        return ContextWindowDetection {
            context_window: Some(value),
            source: Some("configured"),
            detail: Some("Using the user configured context window.".into()),
        };
    }
    if let Some(detected) =
        detect_context_window_from_models(provider, model, base_url, api_key).await
    {
        return detected;
    }
    if let Some(value) = known_context_window(model) {
        return ContextWindowDetection {
            context_window: Some(value),
            source: Some("known_model"),
            detail: Some("Matched built-in model metadata.".into()),
        };
    }
    if let Some(value) = infer_context_window(provider, model) {
        return ContextWindowDetection {
            context_window: Some(value),
            source: Some("model_heuristic"),
            detail: Some("Estimated from provider/model name.".into()),
        };
    }
    ContextWindowDetection {
        context_window: None,
        source: None,
        detail: None,
    }
}

async fn detect_context_window_from_models(
    provider: &str,
    model: &str,
    base_url: Option<&str>,
    api_key: &str,
) -> Option<ContextWindowDetection> {
    let url = models_endpoint(provider, base_url?)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .ok()?;
    let response = client
        .get(&url)
        .headers(model_headers(provider, api_key))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload = response.json::<Value>().await.ok()?;
    extract_context_window_from_payload(&payload, model).map(|tokens| ContextWindowDetection {
        context_window: Some(tokens),
        source: Some("metadata"),
        detail: Some(format!("Detected from `{url}`.")),
    })
}

fn models_endpoint(provider: &str, base_url: &str) -> Option<String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    let provider = provider.to_ascii_lowercase();
    if provider.contains("ollama") || base.ends_with("/api/chat") || base.ends_with("/api/generate")
    {
        let root = base
            .strip_suffix("/api/chat")
            .or_else(|| base.strip_suffix("/api/generate"))
            .unwrap_or(base);
        return Some(format!("{root}/api/tags"));
    }
    let root = base
        .strip_suffix("/chat/completions")
        .or_else(|| base.strip_suffix("/messages"))
        .unwrap_or(base);
    Some(format!("{root}/models"))
}

fn model_headers(provider: &str, api_key: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if api_key.trim().is_empty() {
        return headers;
    }
    if provider.eq_ignore_ascii_case("anthropic") {
        if let Ok(value) = HeaderValue::from_str(api_key) {
            headers.insert("x-api-key", value);
        }
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    } else if let Ok(value) = HeaderValue::from_str(&format!("Bearer {api_key}")) {
        headers.insert(AUTHORIZATION, value);
    }
    headers
}

fn extract_context_window_from_payload(payload: &Value, model: &str) -> Option<u32> {
    let target_aliases = model_aliases(model);
    if target_aliases.is_empty() {
        return None;
    }
    let mut partial_matches = Vec::new();
    for item in model_records(payload) {
        let identities = record_identities(item);
        if identities.is_empty() {
            continue;
        }
        if identities
            .iter()
            .any(|identity| target_aliases.contains(identity))
        {
            if let Some(tokens) = recursive_context_window(item) {
                return Some(tokens);
            }
        } else if identities.iter().any(|identity| {
            target_aliases.iter().any(|alias| {
                identity.ends_with(&format!("/{alias}")) || alias.ends_with(&format!("/{identity}"))
            })
        }) {
            partial_matches.push(item);
        }
    }
    partial_matches
        .into_iter()
        .find_map(recursive_context_window)
}

fn model_records(payload: &Value) -> Vec<&Value> {
    if let Some(items) = payload.as_array() {
        return items.iter().collect();
    }
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    ["data", "models", "result", "items"]
        .iter()
        .filter_map(|key| object.get(*key))
        .filter_map(Value::as_array)
        .flat_map(|items| items.iter())
        .collect()
}

fn record_identities(item: &Value) -> Vec<String> {
    ["id", "model", "name"]
        .iter()
        .filter_map(|key| item.get(*key))
        .filter_map(Value::as_str)
        .flat_map(model_aliases)
        .collect()
}

fn model_aliases(model: &str) -> Vec<String> {
    let value = model.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Vec::new();
    }
    let mut aliases = vec![value.clone()];
    if let Some((_, tail)) = value.split_once('/') {
        aliases.push(tail.to_string());
    }
    if let Some((_, tail)) = value.split_once(':') {
        aliases.push(tail.to_string());
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn recursive_context_window(value: &Value) -> Option<u32> {
    const KEYS: &[&str] = &[
        "context_window",
        "context_window_tokens",
        "context_length",
        "max_context_tokens",
        "max_input_tokens",
        "input_token_limit",
        "max_prompt_tokens",
        "max_model_len",
        "max_sequence_length",
    ];
    match value {
        Value::Object(object) => {
            for key in KEYS {
                if let Some(parsed) = object.get(*key).and_then(coerce_positive_u32) {
                    return Some(parsed);
                }
            }
            object.values().find_map(recursive_context_window)
        }
        Value::Array(items) => items.iter().find_map(recursive_context_window),
        _ => None,
    }
}

fn coerce_positive_u32(value: &Value) -> Option<u32> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|value| u32::try_from(value).ok()),
        Value::String(text) => text.trim().parse::<u32>().ok(),
        _ => None,
    }
    .filter(|value| *value > 0)
}

fn known_context_window(model: &str) -> Option<u32> {
    let model = model.to_ascii_lowercase();
    if model.contains("deepseek-v4") {
        return Some(1_000_000);
    }
    None
}

fn infer_context_window(provider: &str, model: &str) -> Option<u32> {
    let provider = provider.to_ascii_lowercase();
    let model = model.to_ascii_lowercase();
    if provider.contains("anthropic") || model.contains("claude") {
        return Some(200_000);
    }
    if model.contains("gpt-4.1") || model.contains("gpt-4o") || model.starts_with("o3") {
        return Some(128_000);
    }
    if model.contains("gpt-5") || model.starts_with("o4") {
        return Some(128_000);
    }
    if model.contains("deepseek") || model.contains("qwen") || model.contains("glm") {
        return Some(128_000);
    }
    None
}

fn non_empty(value: String, message: &str) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(message.into())
    } else {
        Ok(value)
    }
}

fn non_empty_optional(value: Option<String>, message: &str) -> Result<String, String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| message.into())
}

fn optional_non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn json_error(status: StatusCode, error: String) -> axum::response::Response {
    (status, Json(ErrorResponse { error })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn llm_provider_accepts_openai_compatible_alias() {
        assert_eq!(
            parse_llm_provider("openai-compatible").unwrap(),
            LlmProviderKind::OpenAI
        );
    }

    #[tokio::test]
    async fn context_window_prefers_configured_value() {
        assert_eq!(
            resolve_context_window(
                Some(32_000),
                "anthropic",
                "claude-sonnet-4",
                Some("https://example.invalid/v1"),
                "sk",
            )
            .await,
            ContextWindowDetection {
                context_window: Some(32_000),
                source: Some("configured"),
                detail: Some("Using the user configured context window.".into()),
            }
        );
    }

    #[tokio::test]
    async fn context_window_uses_known_model_before_heuristic() {
        assert_eq!(
            resolve_context_window(None, "openai", "deepseek-v4-flash", None, "sk",)
                .await
                .source,
            Some("known_model")
        );
    }

    #[tokio::test]
    async fn context_window_infers_common_models() {
        assert_eq!(
            resolve_context_window(None, "anthropic", "claude-sonnet-4", None, "sk")
                .await
                .context_window,
            Some(200_000)
        );
        assert_eq!(
            resolve_context_window(None, "openai", "gpt-4o", None, "sk")
                .await
                .context_window,
            Some(128_000)
        );
    }

    #[test]
    fn derives_models_endpoint_from_common_bases() {
        assert_eq!(
            models_endpoint("openai", "https://api.openai.com/v1").unwrap(),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            models_endpoint("openai", "https://host/v1/chat/completions").unwrap(),
            "https://host/v1/models"
        );
        assert_eq!(
            models_endpoint("ollama", "http://localhost:11434/api/chat").unwrap(),
            "http://localhost:11434/api/tags"
        );
    }

    #[test]
    fn extracts_context_window_from_model_metadata() {
        let payload = json!({
            "data": [
                { "id": "other", "max_context_tokens": 4096 },
                { "id": "deepseek-v4-flash", "metadata": { "max_model_len": "1000000" } }
            ]
        });
        assert_eq!(
            extract_context_window_from_payload(&payload, "deepseek-v4-flash"),
            Some(1_000_000)
        );
    }

    #[test]
    fn extracts_context_window_from_provider_prefixed_model() {
        let payload = json!({
            "models": [
                { "name": "Qwen/Qwen3-Coder", "context_length": 262144 }
            ]
        });
        assert_eq!(
            extract_context_window_from_payload(&payload, "Qwen3-Coder"),
            Some(262_144)
        );
    }
}
