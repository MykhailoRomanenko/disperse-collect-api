use log::info;
use routes::api_routes;
use state::AppState;
use tokio::net::TcpListener;

use std::{future::Future, net::SocketAddr};

mod config;
mod contracts;
mod dto;
mod routes;
mod service;
mod state;

pub use config::AppConfig;

pub async fn run(config: AppConfig) -> anyhow::Result<impl Future<Output = anyhow::Result<()>>> {
    let state = AppState::init(config)?;
    let app = api_routes(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Listening on {}", addr);

    let app = async move {
        axum::serve(TcpListener::bind(addr).await?, app)
            .await
            .map_err(Into::into)
    };

    Ok(app)
}
