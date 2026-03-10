//! Request validation middleware and utilities

use crate::error::{ApiError, ApiResult};
use quxtible_core::types::QueryRequest;

/// Validate incoming query request
pub fn validate_query_request(request: &QueryRequest, max_size: usize) -> ApiResult<()> {
    // Validate SQL is not empty
    if request.sql.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "SQL query cannot be empty".to_string(),
        ));
    }

    // Validate SQL size
    if request.sql.len() > max_size {
        return Err(ApiError::InvalidRequest(format!(
            "SQL query exceeds maximum size of {} bytes",
            max_size
        )));
    }

    // Validate natural language query is not empty
    if request.nl_query.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "Natural language query cannot be empty".to_string(),
        ));
    }

    // Validate session ID exists
    if request.context.session_id.is_empty() {
        return Err(ApiError::InvalidRequest(
            "Session ID is required".to_string(),
        ));
    }

    // Validate agent ID exists
    if request.context.agent_id.is_empty() {
        return Err(ApiError::InvalidRequest(
            "Agent ID is required".to_string(),
        ));
    }

    // Validate SQL contains at least a SELECT/INSERT/UPDATE/DELETE
    let upper_sql = request.sql.to_uppercase();
    let valid_operations = ["SELECT", "INSERT", "UPDATE", "DELETE"];
    if !valid_operations.iter().any(|op| upper_sql.contains(op)) {
        return Err(ApiError::InvalidRequest(
            "SQL must contain SELECT, INSERT, UPDATE, or DELETE".to_string(),
        ));
    }

    Ok(())
}

/// Validate batch query requests
pub fn validate_batch_requests(requests: &[QueryRequest], max_size: usize) -> ApiResult<()> {
    if requests.is_empty() {
        return Err(ApiError::InvalidRequest(
            "Batch cannot be empty".to_string(),
        ));
    }

    if requests.len() > 100 {
        return Err(ApiError::InvalidRequest(
            "Batch size cannot exceed 100 queries".to_string(),
        ));
    }

    for (idx, request) in requests.iter().enumerate() {
        validate_query_request(request, max_size).map_err(|e| {
            ApiError::InvalidRequest(format!("Query {} validation failed: {}", idx, e))
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quxtible_core::types::{DatabaseType, QueryContext};

    fn create_test_request(sql: &str, agent_id: &str) -> QueryRequest {
        QueryRequest {
            nl_query: "Test query".to_string(),
            sql: sql.to_string(),
            database: DatabaseType::PostgreSQL,
            schema: None,
            context: QueryContext {
                agent_id: agent_id.to_string(),
                user_id: None,
                session_id: "test-session".to_string(),
                timestamp_ms: 0,
            },
        }
    }

    #[test]
    fn test_valid_select_query() {
        let request = create_test_request("SELECT * FROM users", "agent-1");
        assert!(validate_query_request(&request, 10000).is_ok());
    }

    #[test]
    fn test_empty_sql_rejected() {
        let request = create_test_request("", "agent-1");
        assert!(validate_query_request(&request, 10000).is_err());
    }

    #[test]
    fn test_sql_size_limit() {
        let large_sql = "SELECT * FROM ".to_string() + &"x".repeat(10000);
        let request = create_test_request(&large_sql, "agent-1");
        assert!(validate_query_request(&request, 1000).is_err());
    }

    #[test]
    fn test_missing_session_id() {
        let mut request = create_test_request("SELECT 1", "agent-1");
        request.context.session_id = String::new();
        assert!(validate_query_request(&request, 10000).is_err());
    }

    #[test]
    fn test_missing_agent_id() {
        let mut request = create_test_request("SELECT 1", "");
        assert!(validate_query_request(&request, 10000).is_err());
    }

    #[test]
    fn test_invalid_operation() {
        let request = create_test_request("DROP TABLE users", "agent-1");
        assert!(validate_query_request(&request, 10000).is_err());
    }

    #[test]
    fn test_insert_query() {
        let request = create_test_request("INSERT INTO users VALUES (1, 'test')", "agent-1");
        assert!(validate_query_request(&request, 10000).is_ok());
    }

    #[test]
    fn test_update_query() {
        let request = create_test_request("UPDATE users SET name = 'test' WHERE id = 1", "agent-1");
        assert!(validate_query_request(&request, 10000).is_ok());
    }

    #[test]
    fn test_batch_empty_rejected() {
        assert!(validate_batch_requests(&[], 10000).is_err());
    }

    #[test]
    fn test_batch_too_large() {
        let requests: Vec<_> = (0..101)
            .map(|i| create_test_request("SELECT 1", &format!("agent-{}", i)))
            .collect();
        assert!(validate_batch_requests(&requests, 10000).is_err());
    }

    #[test]
    fn test_batch_valid() {
        let requests: Vec<_> = (0..5)
            .map(|i| create_test_request("SELECT * FROM users", &format!("agent-{}", i)))
            .collect();
        assert!(validate_batch_requests(&requests, 10000).is_ok());
    }
}
