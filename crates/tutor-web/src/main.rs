#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use axum::extract::DefaultBodyLimit;
use tower_http::cors::{Any, CorsLayer};

mod book_store;
mod knowledge_store;
mod memory_store;
mod notebook_store;
mod quiz_store;
mod quiz_tool;
mod routes;
mod session;
mod settings_store;
mod space_tool;
mod stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ServerConfig::from_args(std::env::args().skip(1))?;
    std::fs::create_dir_all(&config.data_dir)?;
    let pool = session::SessionPool::new_with_root(config.data_dir.join("sessions"));
    let knowledge = knowledge_store::KnowledgeStore::new_with_path(
        config.data_dir.join("knowledge-bases.json"),
    );
    let quizzes = std::sync::Arc::new(quiz_store::QuizStore::new_with_path(
        config.data_dir.join("quizzes.json"),
    ));
    let books = std::sync::Arc::new(book_store::BookStore::new_with_path(
        config.data_dir.join("books.json"),
    ));
    let notebook = std::sync::Arc::new(notebook_store::NotebookStore::new_with_path(
        config.data_dir.join("notebook"),
    ));
    if let Err(error) = notebook.start_watcher() {
        eprintln!("failed to start notebook vault watcher: {error}");
    }
    let memory = std::sync::Arc::new(memory_store::MemoryStore::new_with_root(
        config.data_dir.join("memory"),
    ));
    let settings = std::sync::Arc::new(settings_store::SettingsStore::new_with_path(
        config.data_dir.join("settings.json"),
    ));
    let rag_root = config.data_dir.join("rag");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .merge(routes::knowledge::knowledge_router(
            knowledge.clone(),
            memory.clone(),
            rag_root.clone(),
        ))
        .merge(routes::quiz::quiz_router(
            quizzes.clone(),
            knowledge.clone(),
            notebook.clone(),
            memory.clone(),
            rag_root.clone(),
            config.data_dir.join("workflow-sessions").join("quiz"),
        ))
        .merge(routes::books::books_router(books))
        .merge(routes::notebook::notebook_router(
            notebook.clone(),
            memory.clone(),
        ))
        .merge(routes::space::space_router(
            notebook.clone(),
            quizzes.clone(),
        ))
        .merge(routes::memory::memory_router(memory.clone()))
        .merge(routes::settings::settings_router(settings))
        .merge(routes::sessions::sessions_router(
            pool.clone(),
            knowledge.clone(),
        ))
        .merge(routes::ws::ws_router(
            pool.clone(),
            knowledge.clone(),
            memory.clone(),
            notebook.clone(),
            quizzes.clone(),
            rag_root,
        ))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(cors);

    let addr = SocketAddr::new(config.host, config.port);
    println!("tutor-web listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerConfig {
    host: IpAddr,
    port: u16,
    data_dir: PathBuf,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8080,
            data_dir: default_data_dir(),
        }
    }
}

impl ServerConfig {
    fn from_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<Self> {
        let mut config = Self::default();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--host" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--host requires a value"))?;
                    config.host = value.parse()?;
                }
                "--port" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--port requires a value"))?;
                    config.port = value.parse()?;
                }
                "--data-dir" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--data-dir requires a value"))?;
                    config.data_dir = PathBuf::from(value);
                }
                "--help" | "-h" => {
                    println!(
                        "Usage: tutor-web [--host 127.0.0.1] [--port 8080] [--data-dir .llm-tutor]"
                    );
                    std::process::exit(0);
                }
                unknown => anyhow::bail!("unknown argument: {unknown}"),
            }
        }

        Ok(config)
    }
}

fn default_data_dir() -> PathBuf {
    if let Ok(value) = std::env::var("LLM_TUTOR_HOME") {
        let path = PathBuf::from(value);
        if !path.as_os_str().is_empty() {
            return path;
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".llm-tutor")
}

#[cfg(test)]
mod tests {
    use super::{ServerConfig, default_data_dir};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn parses_host_and_port() {
        let config = ServerConfig::from_args([
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            "43127".to_string(),
        ])
        .unwrap();

        assert_eq!(
            config,
            ServerConfig {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 43127,
                data_dir: default_data_dir(),
            }
        );
    }

    #[test]
    fn parses_data_dir() {
        let config = ServerConfig::from_args([
            "--data-dir".to_string(),
            "D:/tmp/llm-tutor-data".to_string(),
        ])
        .unwrap();

        assert_eq!(
            config.data_dir,
            std::path::PathBuf::from("D:/tmp/llm-tutor-data")
        );
    }
}
