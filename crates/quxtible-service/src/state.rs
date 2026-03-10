//! Application state with initialized optimization components

use crate::config::AppConfig;
use quxtible_core::{
    database::{create_connector, DatabaseConnector},
    phase1_cost_estimation::GenericCostEstimator,
    phase2_llm_refinement::ClaudeQueryOptimizer,
    phase3_batch_optimization::BatchOptimizer,
    phase4_autonomous_tuning::TuningAdvisor,
    types::DatabaseType,
};
use std::sync::Arc;

pub struct AppState {
    pub config: AppConfig,
    pub database: Arc<dyn DatabaseConnector>,
    pub cost_estimator: Arc<GenericCostEstimator>,
    pub query_optimizer: Arc<ClaudeQueryOptimizer>,
    pub batch_optimizer: Arc<BatchOptimizer>,
    pub tuning_advisor: Arc<tokio::sync::Mutex<TuningAdvisor>>,
}

impl AppState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        // Initialize database connector
        let database = create_connector(DatabaseType::PostgreSQL, &config.database.url).await?;

        // Initialize cost estimator with configured thresholds
        let cost_estimator = Arc::new(GenericCostEstimator::new(
            database.clone(),
            config.optimization.cost_threshold,
            config.optimization.time_threshold_ms,
        ));

        // Initialize query optimizer (LLM-based)
        let query_optimizer = Arc::new(ClaudeQueryOptimizer::new(
            config.llm.api_key.clone(),
            config.llm.model.clone(),
        ));

        // Initialize batch optimizer with configured threshold
        let batch_optimizer = Arc::new(BatchOptimizer::with_threshold(
            config.optimization.batch_similarity_threshold,
        ));

        // Initialize tuning advisor with configured history size
        let tuning_advisor = Arc::new(tokio::sync::Mutex::new(TuningAdvisor::new(
            config.optimization.tuning_history_size,
        )));

        Ok(Self {
            config,
            database,
            cost_estimator,
            query_optimizer,
            batch_optimizer,
            tuning_advisor,
        })
    }
}
