use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use arrow_array::cast::AsArray;
use arrow_array::types::Float32Type;
use arrow_array::{ArrayRef, FixedSizeListArray, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use futures::future::BoxFuture;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use llm_adapter_embedding::EmbeddingProvider;
use llm_adapter_embedding::openai::OpenAIProvider;
use llm_adapter_embedding::types::EmbeddingRequest;
use serde::{Deserialize, Serialize};

const TABLE_NAME: &str = "chunks";
const DEFAULT_TOP_K: usize = 5;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EmbeddingConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub embeddings_path: Option<String>,
    pub dimensions: Option<usize>,
    pub send_dimensions: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchHit {
    pub id: String,
    pub kb: String,
    pub source: String,
    pub text: String,
    pub score: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceChunk {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IngestProgress {
    pub stage: IngestStage,
    pub message: String,
    pub progress: u8,
    pub chunks: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestStage {
    Chunk,
    Embed,
    Index,
    Done,
}

pub trait KnowledgeRetriever: Send + Sync {
    fn search<'a>(
        &'a self,
        kb: Option<&'a str>,
        query: &'a str,
        top_k: usize,
    ) -> BoxFuture<'a, Result<Vec<SearchHit>>>;
}

#[derive(Clone)]
pub struct LanceDbRag {
    root: PathBuf,
    embedding: EmbeddingConfig,
}

impl LanceDbRag {
    pub fn new(root: impl Into<PathBuf>, embedding: EmbeddingConfig) -> Self {
        Self {
            root: root.into(),
            embedding,
        }
    }

    pub fn default_root() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".llm-tutor")
            .join("rag")
    }

    pub async fn ingest_text(&self, kb: &str, source: &str, text: &str) -> Result<usize> {
        self.ingest_text_with_progress(kb, source, text, |_| {}).await
    }

    pub async fn ingest_text_with_progress(
        &self,
        kb: &str,
        source: &str,
        text: &str,
        mut on_progress: impl FnMut(IngestProgress),
    ) -> Result<usize> {
        on_progress(IngestProgress {
            stage: IngestStage::Chunk,
            message: "Splitting document into chunks".into(),
            progress: 25,
            chunks: None,
        });
        let chunks = chunk_text(text, 900, 160);
        if chunks.is_empty() {
            on_progress(IngestProgress {
                stage: IngestStage::Done,
                message: "Document did not produce chunks".into(),
                progress: 100,
                chunks: Some(0),
            });
            return Ok(0);
        }
        let chunk_count = chunks.len();

        on_progress(IngestProgress {
            stage: IngestStage::Embed,
            message: format!("Embedding {chunk_count} chunks"),
            progress: 45,
            chunks: Some(chunk_count),
        });
        let vectors = self.embed_texts(chunks.clone()).await?;

        on_progress(IngestProgress {
            stage: IngestStage::Index,
            message: "Writing chunks to LanceDB".into(),
            progress: 78,
            chunks: Some(chunk_count),
        });
        let batch = chunks_to_batch(kb, source, chunks, vectors)?;
        let db = connect_db(&self.root).await?;

        match db.open_table(TABLE_NAME).execute().await {
            Ok(table) => {
                table.add(batch).execute().await?;
            }
            Err(_) => {
                db.create_table(TABLE_NAME, batch).execute().await?;
            }
        }

        on_progress(IngestProgress {
            stage: IngestStage::Done,
            message: "Indexed document".into(),
            progress: 92,
            chunks: Some(chunk_count),
        });
        Ok(chunk_count)
    }

    pub async fn delete_source(&self, kb: &str, source: &str) -> Result<usize> {
        let db = connect_db(&self.root).await?;
        let table = match db.open_table(TABLE_NAME).execute().await {
            Ok(table) => table,
            Err(_) => return Ok(0),
        };
        let predicate = source_predicate(kb, source);
        let before = table.count_rows(Some(predicate.clone())).await?;
        if before > 0 {
            table.delete(&predicate).await?;
        }
        Ok(before)
    }

    pub async fn delete_kb(&self, kb: &str) -> Result<usize> {
        let db = connect_db(&self.root).await?;
        let table = match db.open_table(TABLE_NAME).execute().await {
            Ok(table) => table,
            Err(_) => return Ok(0),
        };
        let predicate = kb_predicate(kb);
        let before = table.count_rows(Some(predicate.clone())).await?;
        if before > 0 {
            table.delete(&predicate).await?;
        }
        Ok(before)
    }

    pub async fn chunks_for_source(
        &self,
        kb: &str,
        source: &str,
        limit: usize,
    ) -> Result<Vec<SourceChunk>> {
        let db = connect_db(&self.root).await?;
        let table = match db.open_table(TABLE_NAME).execute().await {
            Ok(table) => table,
            Err(_) => return Ok(vec![]),
        };

        let batches = table
            .query()
            .select(Select::columns(&["id", "text"]))
            .only_if(source_predicate(kb, source))
            .limit(if limit == 0 { 100 } else { limit })
            .execute()
            .await?
            .try_collect::<Vec<_>>()
            .await?;
        Ok(source_chunks_from_batches(&batches))
    }

    async fn embed_texts(&self, input: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if self.embedding.provider != "openai" {
            return Err(anyhow!(
                "unsupported embedding provider `{}`",
                self.embedding.provider
            ));
        }

        let mut builder = OpenAIProvider::builder(self.embedding.api_key.clone());
        if let Some(base_url) = &self.embedding.base_url {
            builder = builder.base_url(base_url.clone());
        }
        if let Some(path) = &self.embedding.embeddings_path {
            builder = builder.embeddings_path(path.clone());
        }
        let provider = builder.build();

        let mut req = EmbeddingRequest::builder(self.embedding.model.clone()).inputs(input);
        if self.embedding.send_dimensions {
            if let Some(dimensions) = self.embedding.dimensions {
                req = req.dimensions(dimensions);
            }
        }

        let response = provider.embed(&req.build()).await?;
        let mut vectors = response.vectors;
        vectors.sort_by_key(|vector| vector.index);
        Ok(vectors.into_iter().map(|vector| vector.embedding).collect())
    }
}

impl KnowledgeRetriever for LanceDbRag {
    fn search<'a>(
        &'a self,
        kb: Option<&'a str>,
        query: &'a str,
        top_k: usize,
    ) -> BoxFuture<'a, Result<Vec<SearchHit>>> {
        Box::pin(async move {
            let mut vectors = self.embed_texts(vec![query.to_string()]).await?;
            let Some(query_vector) = vectors.pop() else {
                return Ok(vec![]);
            };

            let db = connect_db(&self.root).await?;
            let table = match db.open_table(TABLE_NAME).execute().await {
                Ok(table) => table,
                Err(_) => return Ok(vec![]),
            };

            let mut query = table
                .query()
                .select(Select::columns(&["id", "kb", "source", "text"]))
                .limit(if top_k == 0 { DEFAULT_TOP_K } else { top_k })
                .nearest_to(query_vector.as_slice())?
                .column("vector");

            if let Some(kb) = kb.filter(|value| !value.trim().is_empty()) {
                query = query.only_if(format!("kb = '{}'", escape_sql_string(kb)));
            }

            let batches = query.execute().await?.try_collect::<Vec<_>>().await?;
            Ok(search_hits_from_batches(&batches))
        })
    }
}

async fn connect_db(root: &Path) -> Result<lancedb::Connection> {
    tokio::fs::create_dir_all(root).await?;
    let uri = root
        .to_str()
        .ok_or_else(|| anyhow!("RAG database path is not valid UTF-8"))?;
    Ok(lancedb::connect(uri).execute().await?)
}

fn chunks_to_batch(
    kb: &str,
    source: &str,
    chunks: Vec<String>,
    vectors: Vec<Vec<f32>>,
) -> Result<RecordBatch> {
    if chunks.len() != vectors.len() {
        return Err(anyhow!("embedding count does not match chunk count"));
    }
    let dimensions = vectors
        .first()
        .map(|vector| vector.len())
        .ok_or_else(|| anyhow!("embedding response did not contain vectors"))?;
    if dimensions == 0 {
        return Err(anyhow!("embedding vectors must not be empty"));
    }
    if vectors.iter().any(|vector| vector.len() != dimensions) {
        return Err(anyhow!("embedding vectors have inconsistent dimensions"));
    }

    let ids =
        StringArray::from_iter_values((0..chunks.len()).map(|_| uuid::Uuid::new_v4().to_string()));
    let kbs = StringArray::from_iter_values(std::iter::repeat_n(kb.to_string(), chunks.len()));
    let sources =
        StringArray::from_iter_values(std::iter::repeat_n(source.to_string(), chunks.len()));
    let texts = StringArray::from_iter_values(chunks);
    let vectors = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        vectors
            .into_iter()
            .map(|vector| Some(vector.into_iter().map(Some).collect::<Vec<_>>())),
        dimensions as i32,
    );

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("kb", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dimensions as i32,
            ),
            false,
        ),
    ]));

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(ids) as ArrayRef,
            Arc::new(kbs) as ArrayRef,
            Arc::new(sources) as ArrayRef,
            Arc::new(texts) as ArrayRef,
            Arc::new(vectors) as ArrayRef,
        ],
    )
    .context("failed to build RAG record batch")
}

fn search_hits_from_batches(batches: &[RecordBatch]) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    for batch in batches {
        let Some(ids) = batch
            .column_by_name("id")
            .and_then(|array| array.as_string_opt::<i32>())
        else {
            continue;
        };
        let Some(kbs) = batch
            .column_by_name("kb")
            .and_then(|array| array.as_string_opt::<i32>())
        else {
            continue;
        };
        let Some(sources) = batch
            .column_by_name("source")
            .and_then(|array| array.as_string_opt::<i32>())
        else {
            continue;
        };
        let Some(texts) = batch
            .column_by_name("text")
            .and_then(|array| array.as_string_opt::<i32>())
        else {
            continue;
        };
        let scores = batch
            .column_by_name("_distance")
            .and_then(|array| array.as_primitive_opt::<Float32Type>());

        for row in 0..batch.num_rows() {
            hits.push(SearchHit {
                id: ids.value(row).to_string(),
                kb: kbs.value(row).to_string(),
                source: display_source(sources.value(row)),
                text: texts.value(row).to_string(),
                score: scores.map(|array| array.value(row)),
            });
        }
    }
    hits
}

fn source_chunks_from_batches(batches: &[RecordBatch]) -> Vec<SourceChunk> {
    let mut chunks = Vec::new();
    for batch in batches {
        let Some(ids) = batch
            .column_by_name("id")
            .and_then(|array| array.as_string_opt::<i32>())
        else {
            continue;
        };
        let Some(texts) = batch
            .column_by_name("text")
            .and_then(|array| array.as_string_opt::<i32>())
        else {
            continue;
        };
        for row in 0..batch.num_rows() {
            chunks.push(SourceChunk {
                id: ids.value(row).to_string(),
                text: texts.value(row).to_string(),
            });
        }
    }
    chunks
}

fn chunk_text(text: &str, max_chars: usize, overlap_chars: usize) -> Vec<String> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return vec![];
    }

    let chars = normalized.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + max_chars).min(chars.len());
        chunks.push(chars[start..end].iter().collect::<String>());
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(overlap_chars);
    }
    chunks
}

fn source_predicate(kb: &str, source: &str) -> String {
    format!(
        "kb = '{}' AND source = '{}'",
        escape_sql_string(kb),
        escape_sql_string(source)
    )
}

fn kb_predicate(kb: &str) -> String {
    format!("kb = '{}'", escape_sql_string(kb))
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn display_source(value: &str) -> String {
    value
        .split_once("::")
        .map(|(_, source)| source.to_string())
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_text_splits_with_overlap() {
        let chunks = chunk_text("abcdefghijklmnopqrstuvwxyz", 10, 2);
        assert_eq!(chunks[0], "abcdefghij");
        assert_eq!(chunks[1], "ijklmnopqr");
    }

    #[test]
    fn chunks_to_batch_requires_matching_vectors() {
        let err = chunks_to_batch("default", "doc", vec!["hello".into()], vec![]).unwrap_err();
        assert!(err.to_string().contains("embedding count"));
    }
}
