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
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            provider: "duckduckgo".into(),
            base_url: "https://api.duckduckgo.com/".into(),
            api_key: None,
            max_results: 5,
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
        "Search the web for up-to-date information about a topic."
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
                    "items": results,
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

        let mut request = self.client.get(self.config.base_url.trim()).query(&[
            ("q", query),
            ("format", "json"),
            ("no_html", "1"),
            ("no_redirect", "1"),
        ]);
        if let Some(api_key) = self
            .config
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            request = request.header("x-api-key", api_key).bearer_auth(api_key);
        }

        let response = request.send().await?.error_for_status()?;
        let value = response.json::<DuckDuckGoResponse>().await?;
        Ok(parse_duckduckgo_response(
            value,
            self.config.max_results.clamp(1, 10),
        ))
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DuckDuckGoResponse {
    #[serde(default)]
    abstract_text: String,
    #[serde(default)]
    abstract_url: String,
    #[serde(default)]
    heading: String,
    #[serde(default)]
    related_topics: Vec<DuckDuckGoTopic>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DuckDuckGoTopic {
    Topic {
        #[serde(default, rename = "Text")]
        text: String,
        #[serde(default, rename = "FirstURL")]
        first_url: String,
    },
    Group {
        #[serde(default, rename = "Topics")]
        topics: Vec<DuckDuckGoTopic>,
    },
    Other,
}

fn parse_duckduckgo_response(
    response: DuckDuckGoResponse,
    max_results: usize,
) -> Vec<SearchResult> {
    let mut results = Vec::new();
    if !response.abstract_text.trim().is_empty() {
        results.push(SearchResult {
            title: if response.heading.trim().is_empty() {
                "DuckDuckGo abstract".into()
            } else {
                response.heading
            },
            url: response.abstract_url,
            snippet: response.abstract_text,
        });
    }

    push_topics(&mut results, response.related_topics, max_results);
    results.truncate(max_results);
    results
}

fn push_topics(results: &mut Vec<SearchResult>, topics: Vec<DuckDuckGoTopic>, max_results: usize) {
    for topic in topics {
        if results.len() >= max_results {
            return;
        }
        match topic {
            DuckDuckGoTopic::Topic { text, first_url } => {
                if !text.trim().is_empty() {
                    results.push(SearchResult {
                        title: title_from_text(&text),
                        url: first_url,
                        snippet: text,
                    });
                }
            }
            DuckDuckGoTopic::Group { topics } => push_topics(results, topics, max_results),
            DuckDuckGoTopic::Other => {}
        }
    }
}

fn title_from_text(text: &str) -> String {
    text.split(" - ")
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("DuckDuckGo result")
        .chars()
        .take(80)
        .collect()
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
    fn parses_duckduckgo_response() {
        let response = DuckDuckGoResponse {
            abstract_text: "Rust is a programming language.".into(),
            abstract_url: "https://www.rust-lang.org/".into(),
            heading: "Rust".into(),
            related_topics: vec![DuckDuckGoTopic::Topic {
                text: "Rust - A language empowering everyone.".into(),
                first_url: "https://duckduckgo.com/Rust".into(),
            }],
        };

        let results = parse_duckduckgo_response(response, 5);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust");
        assert!(format_results("rust", &results).contains("DuckDuckGo results"));
    }

    #[tokio::test]
    async fn web_search_rejects_empty_query() {
        let tool = WebSearchTool::new();
        let args = serde_json::json!({ "query": "" });
        let err = tool.execute(args, &make_ctx()).await.unwrap_err();
        assert!(err.to_string().contains("query is empty"));
    }
}
