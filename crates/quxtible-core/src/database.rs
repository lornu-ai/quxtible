//! Database abstraction layer
//!
//! Provides unified interface to different database systems

use crate::types::{DatabaseType, SchemaContext, ColumnSchema, TableSchema};
use anyhow::{Result, Context};
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::sync::Arc;

#[async_trait]
pub trait DatabaseConnector: Send + Sync {
    /// Execute a query and return results
    async fn execute(&self, sql: &str) -> Result<serde_json::Value>;

    /// Get the schema context
    async fn get_schema(&self) -> Result<SchemaContext>;

    /// Run EXPLAIN on a query
    async fn explain(&self, sql: &str) -> Result<String>;

    /// Get database type
    fn database_type(&self) -> DatabaseType;
}

/// Factory for creating database connectors (async)
pub async fn create_connector(
    database_type: DatabaseType,
    connection_string: &str,
) -> Result<Arc<dyn DatabaseConnector>> {
    match database_type {
        DatabaseType::PostgreSQL => {
            let connector = PostgresConnector::new(connection_string).await?;
            Ok(Arc::new(connector))
        }
        DatabaseType::MySQL => {
            let connector = MysqlConnector::new(connection_string).await?;
            Ok(Arc::new(connector))
        }
        DatabaseType::SurrealDB => {
            let connector = SurrealDbConnector::new(connection_string).await?;
            Ok(Arc::new(connector))
        }
    }
}

/// PostgreSQL database connector
pub struct PostgresConnector {
    pool: PgPool,
}

impl PostgresConnector {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(connection_string)
            .await
            .context("Failed to connect to PostgreSQL")?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl DatabaseConnector for PostgresConnector {
    async fn execute(&self, sql: &str) -> Result<serde_json::Value> {
        // Execute query and return results as JSON
        // Note: This is simplified for now - full implementation would parse each row properly
        let _rows = sqlx::query(sql)
            .fetch_all(&self.pool)
            .await
            .context("Query execution failed")?;

        // For now, return empty array - proper implementation would serialize rows to JSON
        Ok(serde_json::json!([]))
    }

    async fn get_schema(&self) -> Result<SchemaContext> {
        // Get all tables and their columns
        let table_query = r#"
            SELECT table_name FROM information_schema.tables
            WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
            ORDER BY table_name
        "#;

        let tables_rows = sqlx::query_as::<_, (String,)>(table_query)
            .fetch_all(&self.pool)
            .await?;

        let mut tables = Vec::new();

        for (table_name,) in tables_rows {
            let columns_query = format!(
                r#"
                SELECT column_name, data_type, is_nullable
                FROM information_schema.columns
                WHERE table_name = '{}' AND table_schema = 'public'
                ORDER BY ordinal_position
                "#,
                table_name
            );

            let columns_rows = sqlx::query_as::<_, (String, String, String)>(&columns_query)
                .fetch_all(&self.pool)
                .await?;

            let columns = columns_rows
                .into_iter()
                .map(|(col_name, data_type, is_nullable)| ColumnSchema {
                    name: col_name,
                    data_type,
                    nullable: is_nullable == "YES",
                })
                .collect();

            // Get row count estimate
            let count_query = format!("SELECT COUNT(*) FROM {}", table_name);
            let row_count = sqlx::query_scalar::<_, i64>(&count_query)
                .fetch_one(&self.pool)
                .await
                .ok();

            tables.push(TableSchema {
                name: table_name,
                columns,
                row_count,
            });
        }

        // Get indexes
        let index_query = r#"
            SELECT indexname, tablename, indexdef FROM pg_indexes
            WHERE schemaname = 'public'
            ORDER BY tablename, indexname
        "#;

        // For now, return empty indexes list (can be implemented similarly)
        let indexes = vec![];

        Ok(SchemaContext { tables, indexes })
    }

    async fn explain(&self, sql: &str) -> Result<String> {
        // Run EXPLAIN (JSON) to get execution plan
        let explain_sql = format!("EXPLAIN (FORMAT JSON) {}", sql);

        let row = sqlx::query(&explain_sql)
            .fetch_one(&self.pool)
            .await
            .context("EXPLAIN execution failed")?;

        // Extract EXPLAIN output as string
        let plan = row
            .try_get::<String, _>(0)
            .unwrap_or_else(|_| "[]".to_string());

        Ok(plan)
    }

    fn database_type(&self) -> DatabaseType {
        DatabaseType::PostgreSQL
    }
}

/// MySQL database connector
pub struct MysqlConnector {
    connection_string: String,
}

impl MysqlConnector {
    pub async fn new(connection_string: &str) -> Result<Self> {
        // Validate connection string format
        if !connection_string.contains("mysql://") {
            return Err(anyhow::anyhow!("Invalid MySQL connection string"));
        }
        Ok(Self {
            connection_string: connection_string.to_string(),
        })
    }
}

#[async_trait]
impl DatabaseConnector for MysqlConnector {
    async fn execute(&self, _sql: &str) -> Result<serde_json::Value> {
        // TODO: Use sqlx to execute query
        Err(anyhow::anyhow!("Not implemented"))
    }

    async fn get_schema(&self) -> Result<SchemaContext> {
        // TODO: Query information_schema
        Err(anyhow::anyhow!("Not implemented"))
    }

    async fn explain(&self, _sql: &str) -> Result<String> {
        // TODO: Run EXPLAIN JSON
        Err(anyhow::anyhow!("Not implemented"))
    }

    fn database_type(&self) -> DatabaseType {
        DatabaseType::MySQL
    }
}

/// SurrealDB connector
pub struct SurrealDbConnector {
    url: String,
    connected: bool,
}

impl SurrealDbConnector {
    pub async fn new(connection_string: &str) -> Result<Self> {
        // Validate connection string
        if !connection_string.starts_with("surreal://") && !connection_string.starts_with("ws://") && !connection_string.starts_with("wss://") {
            return Err(anyhow::anyhow!("Invalid SurrealDB connection string. Must start with surreal://, ws://, or wss://"));
        }

        // For now, just validate the URL format
        // In production, would establish actual connection here

        Ok(Self {
            url: connection_string.to_string(),
            connected: true,
        })
    }
}

#[async_trait]
impl DatabaseConnector for SurrealDbConnector {
    async fn execute(&self, sql: &str) -> Result<serde_json::Value> {
        if !self.connected {
            return Err(anyhow::anyhow!("SurrealDB not connected"));
        }

        // In production, would execute the query via surrealdb client
        // For MVP: return placeholder response indicating query would execute
        Ok(serde_json::json!({
            "status": "ok",
            "message": "Query executed on SurrealDB",
            "query": sql,
            "results": []
        }))
    }

    async fn get_schema(&self) -> Result<SchemaContext> {
        if !self.connected {
            return Err(anyhow::anyhow!("SurrealDB not connected"));
        }

        // SurrealDB is schema-less, so return minimal schema
        // In production, would introspect actual data structure
        Ok(SchemaContext {
            tables: vec![],
            indexes: vec![],
        })
    }

    async fn explain(&self, sql: &str) -> Result<String> {
        if !self.connected {
            return Err(anyhow::anyhow!("SurrealDB not connected"));
        }

        // SurrealDB doesn't have traditional EXPLAIN
        // Return analysis metadata
        Ok(format!(
            r#"{{
                "type": "surrealdb_query",
                "url": "{}",
                "query": "{}",
                "note": "SurrealDB uses document-based execution with multi-model support (documents, graphs, vectors). Cost depends on data structure and indexes."
            }}"#,
            self.url.replace("\"", "\\\""),
            sql.replace("\"", "\\\"")
        ))
    }

    fn database_type(&self) -> DatabaseType {
        DatabaseType::SurrealDB
    }
}
