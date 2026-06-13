use std::net::SocketAddr;

use tower_http::cors::{Any, CorsLayer};

mod routes;
mod session;
mod stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = session::SessionPool::new();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .merge(routes::sessions::sessions_router(pool.clone()))
        .merge(routes::ws::ws_router(pool.clone()))
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("tutor-web listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
