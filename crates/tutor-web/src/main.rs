use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::extract::DefaultBodyLimit;
use tower_http::cors::{Any, CorsLayer};

mod book_store;
mod knowledge_store;
mod memory_store;
mod notebook_store;
mod quiz_store;
mod routes;
mod session;
mod stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ServerConfig::from_args(std::env::args().skip(1))?;
    let pool = session::SessionPool::new();
    let knowledge = knowledge_store::KnowledgeStore::new();
    let quizzes = std::sync::Arc::new(quiz_store::QuizStore::new());
    let books = std::sync::Arc::new(book_store::BookStore::new());
    let notebook = std::sync::Arc::new(notebook_store::NotebookStore::new());
    let memory = std::sync::Arc::new(memory_store::MemoryStore::new());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .merge(routes::knowledge::knowledge_router(
            knowledge.clone(),
            memory.clone(),
        ))
        .merge(routes::quiz::quiz_router(
            quizzes,
            knowledge.clone(),
            notebook.clone(),
            memory.clone(),
        ))
        .merge(routes::books::books_router(books))
        .merge(routes::notebook::notebook_router(notebook, memory.clone()))
        .merge(routes::memory::memory_router(memory.clone()))
        .merge(routes::settings::settings_router())
        .merge(routes::sessions::sessions_router(pool.clone(), knowledge))
        .merge(routes::ws::ws_router(pool.clone(), memory.clone()))
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
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8080,
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
                "--help" | "-h" => {
                    println!("Usage: tutor-web [--host 127.0.0.1] [--port 8080]");
                    std::process::exit(0);
                }
                unknown => anyhow::bail!("unknown argument: {unknown}"),
            }
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::ServerConfig;
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
            }
        );
    }
}
