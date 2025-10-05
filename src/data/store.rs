#![cfg(feature = "duckdb")]

//! Lightweight helpers for DuckDB-backed analytical storage.

use std::path::PathBuf;

use anyhow::Result;
use duckdb::Connection;
use tracing::info;

use crate::config::Settings;

/// Wrapper around a DuckDB connection tied to the configured data directory.
pub struct DuckStore {
    pub conn: Connection,
    pub db_path: PathBuf,
}

impl DuckStore {
    /// Open (or create) a DuckDB database within `data/` for ad-hoc queries.
    pub fn open(settings: &Settings) -> Result<Self> {
        let db_path = settings.join_data("rwe.duckdb");
        let conn = Connection::open(&db_path)?;
        info!(path = %db_path.display(), "opened duckdb");
        Ok(Self { conn, db_path })
    }

    /// Register convenience views used by analysts.
    pub fn bootstrap(&self) -> Result<()> {
        self.conn.execute("INSTALL httpfs;", [])?;
        self.conn.execute("LOAD httpfs;", [])?;
        Ok(())
    }
}
