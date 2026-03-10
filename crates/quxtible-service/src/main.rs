mod error;
mod handlers;
mod state;

use axum::{routing::post, Router};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use quxtible_core::types::DatabaseType;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("quxtible=debug".parse()?),
        )
        .init();

    info!("🚀 Quxtible Query Optimization Service starting...");

    // Load configuration from environment
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/quxtible".to_string());
    let claude_api_key = std::env::var("CLAUDE_API_KEY")
        .unwrap_or_else(|_| "sk-test".to_string());
    let claude_model =
        std::env::var("CLAUDE_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());

    info!("📚 Initializing optimization components...");
    info!("  • Database: {}", db_url);
    info!("  • LLM Model: {}", claude_model);

    // Initialize application state with all optimization components
    let state = AppState::new(
        &db_url,
        DatabaseType::PostgreSQL,
        &claude_api_key,
        &claude_model,
    )
    .await?;

    info!("✅ All components initialized");
    info!("  • Phase 1: Cost Estimator");
    info!("  • Phase 2: LLM Query Optimizer");
    info!("  • Phase 3: Batch Optimizer");
    info!("  • Phase 4: Autonomous Tuning Advisor");

    // Build router with all endpoints
    let app = Router::new()
        .route("/healthz", axum::routing::get(handlers::healthz))
        .route("/optimize", post(handlers::optimize_query))
        .route("/estimate-cost", post(handlers::estimate_cost))
        .route("/refine", post(handlers::refine_query))
        .route("/batch-optimize", post(handlers::batch_optimize))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::new(state));

    let addr = "0.0.0.0:8000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("✅ Quxtible listening on http://{}", addr);
    info!("📋 Available endpoints:");
    info!("  GET  /healthz              - Health check");
    info!("  POST /optimize              - Full optimization pipeline (all phases)");
    info!("  POST /estimate-cost         - Phase 1: Cost estimation (EXPLAIN)");
    info!("  POST /refine                - Phase 2: LLM refinement (Claude)");
    info!("  POST /batch-optimize        - Phase 3: Batch optimization (multi-agent)");
    info!("");
    info!("  Phase 4 (Autonomous Tuning) runs as part of /optimize pipeline");

    axum::serve(listener, app).await?;
    Ok(())
}
