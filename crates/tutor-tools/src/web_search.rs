use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebSearchConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub max_results: usize,
    pub fetch_timeout_secs: u64,
    pub max_fetch_chars: usize,
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            provider: "duckduckgo".into(),
            base_url: "https://duckduckgo.com/html/".into(),
            api_key: None,
            max_results: 5,
            fetch_timeout_secs: 12,
            max_fetch_chars: 12_000,
        }
    }
}

pub struct WebSearchTool {
    config: WebSearchConfig,
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self::with_config(WebSearchConfig::default())
    }

    pub fn with_config(config: WebSearchConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for current or externally verifiable information. Use this before answering requests to collect facts, trivia, latest/current information, sources, or details about real-world/public entities, products, games, papers, libraries, events, and online content."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" }
                },
                "required": ["query"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let query = args["query"].as_str().unwrap_or("").to_string();
            if query.trim().is_empty() {
                return Err(ToolError::InvalidArguments("query is empty".into()));
            }

            let results = self
                .search_duckduckgo(&query)
                .await
                .map_err(|err| ToolError::Execution(err.to_string()))?;
            let text = format_results(&query, &results);
            Ok(ToolResult {
                content: vec![ContentBlock::Text { text }],
                details: json!({
                    "query": query,
                    "provider": self.config.provider,
                    "results": results.len(),
                    "items": result_details(&results),
                    "sources": source_details(&results),
                }),
                terminate: false,
            })
        })
    }
}

impl WebSearchTool {
    async fn search_duckduckgo(&self, query: &str) -> anyhow::Result<Vec<SearchResult>> {
        if self.config.provider.trim().to_ascii_lowercase() != "duckduckgo" {
            anyhow::bail!("unsupported web search provider `{}`", self.config.provider);
        }

        let limit = self.config.max_results.clamp(1, 10);
        let response = self
            .client
            .get(self.config.base_url.trim())
            .query(&[("q", query)])
            .header(reqwest::header::USER_AGENT, "llm-tutor/0.1 web_search")
            .send()
            .await?
            .error_for_status()?;
        let html = response.text().await?;
        Ok(parse_duckduckgo_html_results(&html, limit))
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub score: Option<f32>,
    pub source: Option<String>,
}

impl SearchResult {
    fn new(title: impl Into<String>, url: impl Into<String>, snippet: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            snippet: snippet.into(),
            score: None,
            source: Some("duckduckgo".into()),
        }
    }
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn format_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return format!("[WEB] No DuckDuckGo results found for \"{query}\".");
    }

    let body = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            format!(
                "{}. {}\n   URL: {}\n   {}",
                index + 1,
                result.title,
                result.url,
                result.snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("[WEB] DuckDuckGo results for \"{query}\":\n{body}")
}

fn result_details(results: &[SearchResult]) -> serde_json::Value {
    serde_json::Value::Array(
        results
            .iter()
            .map(|result| {
                json!({
                    "title": result.title,
                    "url": result.url,
                    "snippet": result.snippet,
                    "score": result.score,
                    "source": result.source,
                })
            })
            .collect(),
    )
}

fn source_details(results: &[SearchResult]) -> serde_json::Value {
    serde_json::Value::Array(
        results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                json!({
                    "index": index + 1,
                    "kind": "web",
                    "source": result.title,
                    "title": result.title,
                    "url": result.url,
                    "text": result.snippet,
                    "score": result.score,
                })
            })
            .collect(),
    )
}

fn parse_duckduckgo_html_results(html: &str, limit: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut rest = html;

    while results.len() < limit {
        let Some(anchor_pos) = rest.find("result__a") else {
            break;
        };
        rest = &rest[anchor_pos..];

        let Some(href) = attr_value(rest, "href") else {
            rest = &rest["result__a".len()..];
            continue;
        };
        let Some(anchor_end) = rest.find("</a>") else {
            break;
        };
        let title_start = rest.find('>').map(|index| index + 1).unwrap_or(0);
        let title = normalize_text(&strip_html_fragment(&rest[title_start..anchor_end]));
        let url = normalize_duckduckgo_url(&decode_entities(&href));

        let next_anchor = rest[anchor_end..]
            .find("result__a")
            .map(|index| anchor_end + index)
            .unwrap_or(rest.len());
        let block = &rest[..next_anchor];
        let snippet = extract_html_snippet(block).unwrap_or_default();

        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult::new(title, url, snippet));
        }

        rest = &rest[next_anchor..];
    }

    results
}

fn extract_html_snippet(block: &str) -> Option<String> {
    let marker = "result__snippet";
    let pos = block.find(marker)?;
    let part = &block[pos..];
    let start = part.find('>').map(|index| index + 1)?;
    let end = part[start..].find("</").map(|index| start + index)?;
    Some(normalize_text(&strip_html_fragment(&part[start..end])))
}

fn attr_value(input: &str, name: &str) -> Option<String> {
    let marker = format!("{name}=");
    let pos = input.find(&marker)?;
    let value = &input[pos + marker.len()..];
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn normalize_duckduckgo_url(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        if let Some(uddg) = parsed
            .query_pairs()
            .find_map(|(key, value)| (key == "uddg").then_some(value.into_owned()))
        {
            return uddg;
        }
        return parsed.to_string();
    }

    if let Ok(parsed) = reqwest::Url::parse(&format!("https://duckduckgo.com{url}")) {
        if let Some(uddg) = parsed
            .query_pairs()
            .find_map(|(key, value)| (key == "uddg").then_some(value.into_owned()))
        {
            return uddg;
        }
    }

    url.trim().to_string()
}

fn strip_html_fragment(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
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
    fn parses_duckduckgo_html_results() {
        let html = r#"
          <div class="result">
            <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fdocs&amp;rut=abc">
              Example <b>Docs</b>
            </a>
            <a class="result__snippet">Official documentation &amp; examples.</a>
          </div>
        "#;

        let results = parse_duckduckgo_html_results(html, 5);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example Docs");
        assert_eq!(results[0].url, "https://example.com/docs");
        assert_eq!(results[0].snippet, "Official documentation & examples.");
    }

    #[tokio::test]
    async fn web_search_rejects_empty_query() {
        let tool = WebSearchTool::new();
        let args = serde_json::json!({ "query": "" });
        let err = tool.execute(args, &make_ctx()).await.unwrap_err();
        assert!(err.to_string().contains("query is empty"));
    }
}
