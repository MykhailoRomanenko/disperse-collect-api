use axum::Router;
use routes::api_routes;
use state::AppState;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

use std::{future::Future, net::SocketAddr};

mod config;
mod contracts;
mod dto;
mod routes;
mod service;
mod state;

pub use config::AppConfig;

pub async fn run(config: AppConfig) -> anyhow::Result<impl Future<Output = anyhow::Result<()>>> {
    let port = config.port;

    let state = AppState::init(config)?;
    let app = Router::new()
        .nest("/api", api_routes(state))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("Listening on {}", addr);

    let app = async move {
        axum::serve(TcpListener::bind(addr).await?, app.into_make_service())
            .await
            .map_err(Into::into)
    };

    Ok(app)
}
