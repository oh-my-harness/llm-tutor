use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;
use std::time::Duration;

use crate::text_decode::decode_response_text;
use crate::web_search::WebSearchConfig;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

pub struct WebFetchTool {
    config: WebSearchConfig,
    client: reqwest::Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self::with_config(WebSearchConfig::default())
    }

    pub fn with_config(config: WebSearchConfig) -> Self {
        let timeout = Duration::from_secs(config.fetch_timeout_secs.clamp(3, 60));
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(timeout)
                .user_agent("llm-tutor/0.1 web_fetch")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a web page URL and extract readable page text for citation-backed answers."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "HTTP or HTTPS URL to fetch" }
                },
                "required": ["url"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let url = args["url"].as_str().unwrap_or("").trim().to_string();
            if url.is_empty() {
                return Err(ToolError::InvalidArguments("url is empty".into()));
            }

            let fetched = self
                .fetch_url(&url)
                .await
                .map_err(|err| ToolError::Execution(err.to_string()))?;
            let text = format_fetch_result(&fetched);
            Ok(ToolResult {
                content: vec![ContentBlock::Text { text }],
                details: json!({
                    "url": fetched.url,
                    "title": fetched.title,
                    "description": fetched.description,
                    "content_type": fetched.content_type,
                    "truncated": fetched.truncated,
                    "chars": fetched.text.chars().count(),
                    "sources": [{
                        "index": 1,
                        "kind": "web",
                        "source": fetched.title,
                        "title": fetched.title,
                        "url": fetched.url,
                        "text": fetched.text,
                        "score": null,
                    }],
                }),
                terminate: false,
            })
        })
    }
}

impl WebFetchTool {
    async fn fetch_url(&self, url: &str) -> anyhow::Result<FetchedPage> {
        let parsed = reqwest::Url::parse(url)?;
        match parsed.scheme() {
            "http" | "https" => {}
            other => anyhow::bail!("unsupported URL scheme `{other}`; only http/https are allowed"),
        }

        let response = self.client.get(parsed).send().await?;
        let final_url = response.url().to_string();
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("fetch failed with HTTP {status}");
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        if !content_type.is_empty()
            && !content_type.contains("text/html")
            && !content_type.contains("text/plain")
            && !content_type.contains("application/xhtml")
        {
            anyhow::bail!("unsupported content type `{content_type}`");
        }

        let headers = response.headers().clone();
        let bytes = response.bytes().await?;
        let body = decode_response_text(&headers, &bytes);
        let page = extract_page_text(
            &body,
            &final_url,
            &content_type,
            self.config.max_fetch_chars,
        );
        Ok(page)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FetchedPage {
    title: String,
    description: String,
    url: String,
    content_type: String,
    text: String,
    truncated: bool,
}

fn extract_page_text(body: &str, url: &str, content_type: &str, max_chars: usize) -> FetchedPage {
    let title = html_tag_text(body, "title").unwrap_or_else(|| url.to_string());
    let description = html_meta_content(body, "description").unwrap_or_default();
    let cleaned = if content_type.contains("text/plain") {
        normalize_whitespace(body)
    } else {
        normalize_whitespace(&strip_html(body))
    };
    let max_chars = max_chars.clamp(1_000, 60_000);
    let (text, truncated) = truncate_chars(&cleaned, max_chars);

    FetchedPage {
        title: decode_entities(&title),
        description: decode_entities(&description),
        url: url.to_string(),
        content_type: content_type.to_string(),
        text,
        truncated,
    }
}

fn format_fetch_result(page: &FetchedPage) -> String {
    let mut lines = vec![
        format!("[WEB] Fetched: {}", page.title),
        format!("URL: {}", page.url),
    ];
    if !page.description.trim().is_empty() {
        lines.push(format!("Description: {}", page.description));
    }
    if page.truncated {
        lines.push("Note: content was truncated.".into());
    }
    lines.push(format!("Content:\n{}", page.text));
    lines.join("\n")
}

fn strip_html(input: &str) -> String {
    let without_scripts = remove_tag_block(input, "script");
    let without_styles = remove_tag_block(&without_scripts, "style");
    let without_svg = remove_tag_block(&without_styles, "svg");

    let mut out = String::with_capacity(without_svg.len());
    let mut in_tag = false;
    for ch in without_svg.chars() {
        match ch {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_entities(&out)
}

fn remove_tag_block(input: &str, tag: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    let open = format!("<{tag}");
    let close = format!("</{tag}>");

    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(start) = lower.find(&open) else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        let after_start = &rest[start..];
        let after_lower = after_start.to_ascii_lowercase();
        let Some(end) = after_lower.find(&close) else {
            break;
        };
        rest = &after_start[end + close.len()..];
    }

    out
}

fn html_tag_text(input: &str, tag: &str) -> Option<String> {
    let lower = input.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = lower.find(&open)?;
    let after_open = input[start..].find('>')? + start + 1;
    let end = lower[after_open..].find(&close)? + after_open;
    Some(normalize_whitespace(&strip_html(&input[after_open..end])))
}

fn html_meta_content(input: &str, name: &str) -> Option<String> {
    for part in input.split('<') {
        let lower = part.to_ascii_lowercase();
        if !lower.starts_with("meta") || !lower.contains(&format!("name=\"{name}\"")) {
            continue;
        }
        let content_index = lower.find("content=")?;
        let value = &part[content_index + "content=".len()..];
        return quoted_attr_value(value).map(|text| text.to_string());
    }
    None
}

fn quoted_attr_value(value: &str) -> Option<&str> {
    let mut chars = value.chars();
    let quote = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }
    (text.chars().take(max_chars).collect(), true)
}

fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use llm_harness_types::UnsupportedEnv;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn make_ctx() -> ToolContext {
        let (tx, _rx) = mpsc::channel(1);
        ToolContext {
            env: Arc::new(UnsupportedEnv::new()),
            abort: CancellationToken::new(),
            tool_use_id: "test-id".into(),
            turn_index: 0,
            assistant_message: Arc::new(llm_harness_types::AssistantMessage {
                kind: llm_harness_types::AssistantMessageKind::FinalAnswer,
                message_id: "test-message".into(),
                turn_id: "test-turn".into(),
                content: vec![],
                usage: None,
                stop_reason: None,
                timestamp: Utc::now(),
                provider: None,
                api: None,
                model: None,
                error_message: None,
            }),
            update_tx: tx,
        }
    }

    #[test]
    fn extracts_readable_html() {
        let html = r#"
            <html><head>
              <title>Example &amp; Test</title>
              <meta name="description" content="Short page">
              <style>.hidden { display: none; }</style>
            </head><body>
              <script>alert(1)</script>
              <main><h1>Hello</h1><p>Readable text.</p></main>
            </body></html>
        "#;

        let page = extract_page_text(html, "https://example.com", "text/html", 10_000);

        assert_eq!(page.title, "Example & Test");
        assert_eq!(page.description, "Short page");
        assert!(page.text.contains("Hello Readable text."));
        assert!(!page.text.contains("alert"));
    }

    #[tokio::test]
    async fn web_fetch_rejects_empty_url() {
        let tool = WebFetchTool::new();
        let args = serde_json::json!({ "url": "" });
        let err = tool.execute(args, &make_ctx()).await.unwrap_err();
        assert!(err.to_string().contains("url is empty"));
    }
}
