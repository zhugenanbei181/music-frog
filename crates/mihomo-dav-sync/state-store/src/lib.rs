use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct SyncStateRow {
    pub path: String,
    pub last_etag: String,
    pub last_hash: String,
    pub last_sync_at: DateTime<Utc>,
    pub is_tombstone: i64, // SQLite 不直接支持 bool，用 i64
}

pub struct StateStore {
    pool: SqlitePool,
}

impl StateStore {
    pub async fn new(db_path: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path))?
            .create_if_missing(true);
        
        let pool = SqlitePool::connect_with(options).await?;
        Self::init(&pool).await?;
        Ok(Self { pool })
    }

    /// Create an in-memory store for testing
    #[cfg(test)]
    pub async fn new_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePool::connect_with(options).await?;
        Self::init(&pool).await?;
        Ok(Self { pool })
    }

    async fn init(pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sync_state (
                path TEXT PRIMARY KEY,
                last_etag TEXT NOT NULL,
                last_hash TEXT NOT NULL,
                last_sync_at DATETIME NOT NULL,
                is_tombstone INTEGER NOT NULL DEFAULT 0
            )"
        ).execute(pool).await?;
        Ok(())
    }

    pub async fn get_all_states(&self) -> Result<Vec<SyncStateRow>> {
        let rows = sqlx::query_as::<_, SyncStateRow>("SELECT * FROM sync_state")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get_state(&self, path: &str) -> Result<Option<SyncStateRow>> {
        let row = sqlx::query_as::<_, SyncStateRow>("SELECT * FROM sync_state WHERE path = ?")
            .bind(path)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn upsert_state(&self, row: SyncStateRow) -> Result<()> {
        sqlx::query(
            "INSERT INTO sync_state (path, last_etag, last_hash, last_sync_at, is_tombstone)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(path) DO UPDATE SET
                last_etag = excluded.last_etag,
                last_hash = excluded.last_hash,
                last_sync_at = excluded.last_sync_at,
                is_tombstone = excluded.is_tombstone"
        )
        .bind(row.path)
        .bind(row.last_etag)
        .bind(row.last_hash)
        .bind(row.last_sync_at)
        .bind(row.is_tombstone)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_state(&self, path: &str) -> Result<()> {
        sqlx::query("DELETE FROM sync_state WHERE path = ?")
            .bind(path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_tombstone(&self, path: &str) -> Result<()> {
        sqlx::query("UPDATE sync_state SET is_tombstone = 1, last_sync_at = ? WHERE path = ?")
            .bind(Utc::now())
            .bind(path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_row(path: &str) -> SyncStateRow {
        SyncStateRow {
            path: path.to_string(),
            last_etag: "etag123".to_string(),
            last_hash: "hash456".to_string(),
            last_sync_at: Utc::now(),
            is_tombstone: 0,
        }
    }

    #[tokio::test]
    async fn test_create_in_memory_store() {
        let store = StateStore::new_in_memory().await;
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_upsert_and_get_state() {
        let store = StateStore::new_in_memory().await.unwrap();
        let row = make_test_row("config.yaml");
        
        store.upsert_state(row.clone()).await.unwrap();
        
        let retrieved = store.get_state("config.yaml").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.path, "config.yaml");
        assert_eq!(retrieved.last_etag, "etag123");
        assert_eq!(retrieved.last_hash, "hash456");
    }

    #[tokio::test]
    async fn test_get_nonexistent_state() {
        let store = StateStore::new_in_memory().await.unwrap();
        
        let result = store.get_state("nonexistent.yaml").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_upsert_updates_existing() {
        let store = StateStore::new_in_memory().await.unwrap();
        
        let row1 = SyncStateRow {
            path: "test.yaml".to_string(),
            last_etag: "old_etag".to_string(),
            last_hash: "old_hash".to_string(),
            last_sync_at: Utc::now(),
            is_tombstone: 0,
        };
        store.upsert_state(row1).await.unwrap();
        
        let row2 = SyncStateRow {
            path: "test.yaml".to_string(),
            last_etag: "new_etag".to_string(),
            last_hash: "new_hash".to_string(),
            last_sync_at: Utc::now(),
            is_tombstone: 0,
        };
        store.upsert_state(row2).await.unwrap();
        
        let retrieved = store.get_state("test.yaml").await.unwrap().unwrap();
        assert_eq!(retrieved.last_etag, "new_etag");
        assert_eq!(retrieved.last_hash, "new_hash");
    }

    #[tokio::test]
    async fn test_delete_state() {
        let store = StateStore::new_in_memory().await.unwrap();
        let row = make_test_row("to_delete.yaml");
        
        store.upsert_state(row).await.unwrap();
        assert!(store.get_state("to_delete.yaml").await.unwrap().is_some());
        
        store.delete_state("to_delete.yaml").await.unwrap();
        assert!(store.get_state("to_delete.yaml").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_all_states() {
        let store = StateStore::new_in_memory().await.unwrap();
        
        store.upsert_state(make_test_row("file1.yaml")).await.unwrap();
        store.upsert_state(make_test_row("file2.yaml")).await.unwrap();
        store.upsert_state(make_test_row("file3.yaml")).await.unwrap();
        
        let all = store.get_all_states().await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_mark_tombstone() {
        let store = StateStore::new_in_memory().await.unwrap();
        let row = make_test_row("tombstone.yaml");
        
        store.upsert_state(row).await.unwrap();
        store.mark_tombstone("tombstone.yaml").await.unwrap();
        
        let retrieved = store.get_state("tombstone.yaml").await.unwrap().unwrap();
        assert_eq!(retrieved.is_tombstone, 1);
    }
}