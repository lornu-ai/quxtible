mod config;
mod error;
mod handlers;
mod health;
mod metrics;
mod state;
mod validation;

use axum::{routing::post, Router};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use config::AppConfig;
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
    let config = AppConfig::from_env();
    match config {
        Ok(cfg) => {
            cfg.validate()?;
            info!("📚 Configuration loaded and validated");

            // Log server configuration
            info!("Server Configuration:");
            info!("  • Host: {}", cfg.server.host);
            info!("  • Port: {}", cfg.server.port);
            info!("  • Request Timeout: {}s", cfg.server.request_timeout_secs);
            info!("  • Max Request Size: {} bytes", cfg.server.max_request_body_bytes);

            // Log database configuration
            info!("Database Configuration:");
            info!("  • Type: {}", cfg.database.database_type.to_uppercase());
            info!("  • URL: {}", cfg.database.url);
            info!("  • Max Connections: {}", cfg.database.max_connections);
            info!("  • Min Connections: {}", cfg.database.min_connections);

            // Log LLM configuration
            info!("LLM Configuration:");
            info!("  • Model: {}", cfg.llm.model);
            info!("  • Max Tokens: {}", cfg.llm.max_tokens);

            // Log optimization parameters
            info!("Optimization Parameters:");
            info!("  • Cost Threshold: {}", cfg.optimization.cost_threshold);
            info!("  • Time Threshold: {}ms", cfg.optimization.time_threshold_ms);
            info!("  • Batch Similarity: {}", cfg.optimization.batch_similarity_threshold);

            // Log rate limiting
            if cfg.rate_limit.enabled {
                info!("Rate Limiting: {} req/min", cfg.rate_limit.requests_per_minute);
            } else {
                info!("Rate Limiting: DISABLED");
            }

            info!("📚 Initializing optimization components...");

            // Initialize application state with all optimization components
            let state = AppState::new(cfg).await?;

            info!("✅ All components initialized");
            info!("  ✓ Phase 1: Cost Estimator");
            info!("  ✓ Phase 2: LLM Query Optimizer");
            info!("  ✓ Phase 3: Batch Optimizer");
            info!("  ✓ Phase 4: Autonomous Tuning Advisor");

            // Get server config for binding
            let server_config = state.config.server.clone();

            // Build router with all endpoints
            let app = Router::new()
                // Health endpoints (for K8s probes)
                .route("/healthz", axum::routing::get(handlers::healthz))
                .route("/health", axum::routing::get(health::health_check))
                .route("/ready", axum::routing::get(health::readiness_check))
                .route("/live", axum::routing::get(health::liveness_check))

                // Optimization endpoints
                .route("/optimize", post(handlers::optimize_query))
                .route("/estimate-cost", post(handlers::estimate_cost))
                .route("/refine", post(handlers::refine_query))
                .route("/batch-optimize", post(handlers::batch_optimize))

                // Observability endpoints
                .route("/metrics", axum::routing::get(handlers::get_metrics))

                .layer(CorsLayer::permissive())
                .layer(TraceLayer::new_for_http())
                .with_state(Arc::new(state));

            let addr = format!("{}:{}", server_config.host, server_config.port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;

            info!("✅ Quxtible listening on http://{}", addr);
            info!("📋 Available endpoints:");
            info!("");
            info!("Health & Observability:");
            info!("  GET  /healthz              - Quick health check");
            info!("  GET  /health               - Detailed component status");
            info!("  GET  /ready                - Kubernetes readiness probe");
            info!("  GET  /live                 - Kubernetes liveness probe");
            info!("  GET  /metrics              - Performance metrics (JSON)");
            info!("");
            info!("Optimization Endpoints:");
            info!("  POST /optimize              - Full pipeline (all 4 phases)");
            info!("  POST /estimate-cost         - Phase 1: Cost estimation");
            info!("  POST /refine                - Phase 2: LLM refinement");
            info!("  POST /batch-optimize        - Phase 3: Batch optimization");
            info!("");
            info!("  Phase 4 (Autonomous Tuning) integrated into /optimize pipeline");
            info!("");

            axum::serve(listener, app).await?;
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Failed to load configuration: {}", e);
            Err(e.into())
        }
    }
}
