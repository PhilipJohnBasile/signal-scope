//! HTTP layer exposing computed signals and static UI.

pub mod routes;
pub mod types;

use std::net::SocketAddr;

use anyhow::Result;
use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::info;

use crate::config::Settings;

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
}

pub async fn serve(settings: Settings, host: String, port: u16) -> Result<()> {
    let state = AppState {
        settings: settings.clone(),
    };
    let static_dir = ServeDir::new("src/ui/static");
    let router = Router::new()
        .route("/signals", get(routes::list_signals))
        .route("/events/:drug_id", get(routes::list_events))
        .fallback_service(static_dir)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    info!(%addr, "serving rwe-assistant API");
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}
