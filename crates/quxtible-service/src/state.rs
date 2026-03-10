//! Application state with initialized optimization components

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
    pub database: Arc<dyn DatabaseConnector>,
    pub cost_estimator: Arc<GenericCostEstimator>,
    pub query_optimizer: Arc<ClaudeQueryOptimizer>,
    pub batch_optimizer: Arc<BatchOptimizer>,
    pub tuning_advisor: Arc<tokio::sync::Mutex<TuningAdvisor>>,
}

impl AppState {
    pub async fn new(
        database_url: &str,
        database_type: DatabaseType,
        claude_api_key: &str,
        claude_model: &str,
    ) -> anyhow::Result<Self> {
        // Initialize database connector
        let database = create_connector(database_type, database_url).await?;

        // Initialize cost estimator
        let cost_estimator = Arc::new(GenericCostEstimator::new(
            database.clone(),
            1000.0,  // cost threshold
            100.0,   // time threshold (ms)
        ));

        // Initialize query optimizer (LLM-based)
        let query_optimizer = Arc::new(ClaudeQueryOptimizer::new(
            claude_api_key.to_string(),
            claude_model.to_string(),
        ));

        // Initialize batch optimizer
        let batch_optimizer = Arc::new(BatchOptimizer::with_threshold(0.7));

        // Initialize tuning advisor (RL-based learning agent)
        let tuning_advisor = Arc::new(tokio::sync::Mutex::new(TuningAdvisor::new(1000)));

        Ok(Self {
            database,
            cost_estimator,
            query_optimizer,
            batch_optimizer,
            tuning_advisor,
        })
    }
}
