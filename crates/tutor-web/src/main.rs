use std::net::SocketAddr;

use axum::extract::DefaultBodyLimit;
use tower_http::cors::{Any, CorsLayer};

mod knowledge_store;
mod quiz_store;
mod routes;
mod session;
mod stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = session::SessionPool::new();
    let knowledge = knowledge_store::KnowledgeStore::new();
    let quizzes = std::sync::Arc::new(quiz_store::QuizStore::new());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .merge(routes::knowledge::knowledge_router(knowledge.clone()))
        .merge(routes::quiz::quiz_router(quizzes, knowledge.clone()))
        .merge(routes::sessions::sessions_router(pool.clone(), knowledge))
        .merge(routes::ws::ws_router(pool.clone()))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("tutor-web listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
