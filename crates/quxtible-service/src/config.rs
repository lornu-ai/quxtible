//! Configuration management for production deployment

use serde::{Deserialize, Serialize};
use std::env;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Server configuration
    pub server: ServerConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// LLM (Claude) configuration
    pub llm: LlmConfig,
    /// Optimization parameters
    pub optimization: OptimizationConfig,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub request_timeout_secs: u64,
    pub max_request_body_bytes: usize,
}

/// Database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_secs: u64,
}

/// LLM (Claude API) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: usize,
    pub timeout_secs: u64,
}

/// Optimization phase parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    /// Phase 1: Cost thresholds
    pub cost_threshold: f64,
    pub time_threshold_ms: f64,
    /// Phase 3: Batch similarity threshold
    pub batch_similarity_threshold: f32,
    /// Phase 4: Tuning advisor history size
    pub tuning_history_size: usize,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per minute per client
    pub requests_per_minute: u32,
    /// Enable rate limiting
    pub enabled: bool,
}

impl AppConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            server: ServerConfig {
                host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("SERVER_PORT")
                    .unwrap_or_else(|_| "8000".to_string())
                    .parse()
                    .unwrap_or(8000),
                request_timeout_secs: env::var("REQUEST_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "60".to_string())
                    .parse()
                    .unwrap_or(60),
                max_request_body_bytes: env::var("MAX_REQUEST_BODY_BYTES")
                    .unwrap_or_else(|_| "1048576".to_string()) // 1MB default
                    .parse()
                    .unwrap_or(1_048_576),
            },
            database: DatabaseConfig {
                url: env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "postgresql://localhost/quxtible".to_string()),
                max_connections: env::var("DB_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
                min_connections: env::var("DB_MIN_CONNECTIONS")
                    .unwrap_or_else(|_| "2".to_string())
                    .parse()
                    .unwrap_or(2),
                connection_timeout_secs: env::var("DB_CONNECTION_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
            },
            llm: LlmConfig {
                api_key: env::var("CLAUDE_API_KEY")
                    .unwrap_or_else(|_| "sk-test".to_string()),
                model: env::var("CLAUDE_MODEL")
                    .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string()),
                max_tokens: env::var("CLAUDE_MAX_TOKENS")
                    .unwrap_or_else(|_| "2000".to_string())
                    .parse()
                    .unwrap_or(2000),
                timeout_secs: env::var("CLAUDE_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
            },
            optimization: OptimizationConfig {
                cost_threshold: env::var("COST_THRESHOLD")
                    .unwrap_or_else(|_| "1000.0".to_string())
                    .parse()
                    .unwrap_or(1000.0),
                time_threshold_ms: env::var("TIME_THRESHOLD_MS")
                    .unwrap_or_else(|_| "100.0".to_string())
                    .parse()
                    .unwrap_or(100.0),
                batch_similarity_threshold: env::var("BATCH_SIMILARITY_THRESHOLD")
                    .unwrap_or_else(|_| "0.7".to_string())
                    .parse()
                    .unwrap_or(0.7),
                tuning_history_size: env::var("TUNING_HISTORY_SIZE")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .unwrap_or(1000),
            },
            rate_limit: RateLimitConfig {
                requests_per_minute: env::var("RATE_LIMIT_RPM")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse()
                    .unwrap_or(300),
                enabled: env::var("RATE_LIMIT_ENABLED")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
            },
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.server.port == 0 {
            return Err(anyhow::anyhow!("Invalid server port"));
        }
        if self.database.url.is_empty() {
            return Err(anyhow::anyhow!("Database URL is required"));
        }
        if self.llm.api_key.is_empty() || self.llm.api_key == "sk-test" {
            eprintln!("⚠️  WARNING: Using test Claude API key. Set CLAUDE_API_KEY for production.");
        }
        if self.optimization.cost_threshold < 0.0 {
            return Err(anyhow::anyhow!("Cost threshold must be positive"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env_defaults() {
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8000);
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.optimization.cost_threshold, 1000.0);
    }

    #[test]
    fn test_config_validation_passes() {
        let config = AppConfig::from_env().unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_rate_limit_defaults() {
        let config = AppConfig::from_env().unwrap();
        assert!(config.rate_limit.enabled);
        assert_eq!(config.rate_limit.requests_per_minute, 300);
    }
}
