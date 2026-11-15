#![forbid(unsafe_code)]

use crate::model::GameState;
use rusqlite::{params, Connection};
use std::fs;
use std::path::Path;

pub struct SaveStore {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct SaveSlotSummary {
    pub slot_name: String,
    pub updated_utc: String,
    pub credits: f64,
    pub shipped: u64,
    pub robot_count: usize,
}

impl SaveStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("SQLite open error: {e}"))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| format!("WAL pragma error: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS save_slots (
                slot_name   TEXT PRIMARY KEY,
                updated_utc TEXT NOT NULL,
                credits     REAL NOT NULL,
                shipped     INTEGER NOT NULL,
                robot_count INTEGER NOT NULL,
                state_json  TEXT NOT NULL
            );",
        )
        .map_err(|e| format!("Schema init error: {e}"))?;
        Ok(Self { conn })
    }

    pub fn save_slot(&self, slot_name: &str, state: &GameState) -> Result<(), String> {
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| format!("JSON serialize error: {e}"))?;
        let ts = format!("T+{:.0}s", state.elapsed_secs);
        self.conn
            .execute(
                "INSERT INTO save_slots (slot_name, updated_utc, credits, shipped, robot_count, state_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(slot_name) DO UPDATE SET
                    updated_utc = excluded.updated_utc,
                    credits     = excluded.credits,
                    shipped     = excluded.shipped,
                    robot_count = excluded.robot_count,
                    state_json  = excluded.state_json;",
                params![
                    slot_name,
                    ts,
                    state.credits,
                    state.total_products_shipped as i64,
                    state.robots.len() as i64,
                    json
                ],
            )
            .map_err(|e| format!("Save slot error: {e}"))?;
        Ok(())
    }

    pub fn load_slot(&self, slot_name: &str) -> Result<GameState, String> {
        let json: String = self
            .conn
            .query_row(
                "SELECT state_json FROM save_slots WHERE slot_name = ?1",
                params![slot_name],
                |row| row.get(0),
            )
            .map_err(|e| format!("Load slot '{slot_name}' failed: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("Deserialize error: {e}"))
    }

    pub fn list_slots(&self) -> Result<Vec<SaveSlotSummary>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT slot_name, updated_utc, credits, shipped, robot_count
                 FROM save_slots ORDER BY rowid DESC",
            )
            .map_err(|e| format!("Prepare list error: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SaveSlotSummary {
                    slot_name: row.get(0)?,
                    updated_utc: row.get(1)?,
                    credits: row.get(2)?,
                    shipped: row.get::<_, i64>(3)? as u64,
                    robot_count: row.get::<_, i64>(4)? as usize,
                })
            })
            .map_err(|e| format!("Query slots error: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("Row read error: {e}"))?);
        }
        Ok(out)
    }
}

pub fn export_json_file(path: impl AsRef<Path>, state: &GameState) -> Result<(), String> {
    let json =
        serde_json::to_string_pretty(state).map_err(|e| format!("Serialize error: {e}"))?;
    fs::write(path, json).map_err(|e| format!("File write error: {e}"))
}

pub fn import_json_file(path: impl AsRef<Path>) -> Result<GameState, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("File read error: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_wal_roundtrip() {
        let store = SaveStore::open(":memory:").expect("in-memory sqlite");
        let mut state = GameState::new_starter_factory();
        state.credits = 777.5;
        store.save_slot("autosave", &state).expect("save");
        let loaded = store.load_slot("autosave").expect("load");
        assert!((loaded.credits - 777.5).abs() < 1e-6);
    }
}
