// Copyright (C) 2026 CRV Science Network (https://crvscience.com)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://gnu.org>.

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
        let slot = slot_name.trim();
        if slot.is_empty() {
            return Err("Slot name cannot be empty.".to_string());
        }
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
                    slot,
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
        let slot = slot_name.trim();
        if slot.is_empty() {
            return Err("Slot name cannot be empty.".to_string());
        }
        let json: String = self
            .conn
            .query_row(
                "SELECT state_json FROM save_slots WHERE slot_name = ?1",
                params![slot],
                |row| row.get(0),
            )
            .map_err(|e| format!("Load slot '{slot}' failed: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("Deserialize error: {e}"))
    }

    pub fn delete_slot(&self, slot_name: &str) -> Result<(), String> {
        let slot = slot_name.trim();
        if slot.is_empty() {
            return Err("Slot name cannot be empty.".to_string());
        }
        let rows = self
            .conn
            .execute("DELETE FROM save_slots WHERE slot_name = ?1", params![slot])
            .map_err(|e| format!("Delete slot '{slot}' failed: {e}"))?;
        if rows == 0 {
            return Err(format!("Slot '{slot}' not found."));
        }
        Ok(())
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

    #[test]
    fn list_slots_and_delete_slot_lifecycle() {
        let store = SaveStore::open(":memory:").expect("in-memory sqlite");
        let mut s1 = GameState::new_starter_factory();
        s1.credits = 150.0;
        s1.total_products_shipped = 4;
        store.save_slot("slot_a", &s1).expect("save slot_a");

        let mut s2 = GameState::new_starter_factory();
        s2.credits = 920.0;
        s2.total_products_shipped = 12;
        store.save_slot("slot_b", &s2).expect("save slot_b");

        let slots = store.list_slots().expect("list_slots");
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].slot_name, "slot_b");
        assert!((slots[0].credits - 920.0).abs() < 1e-6);
        assert_eq!(slots[0].shipped, 12);
        assert_eq!(slots[1].slot_name, "slot_a");
        assert!((slots[1].credits - 150.0).abs() < 1e-6);

        store.delete_slot("slot_a").expect("delete slot_a");
        let remaining = store.list_slots().expect("list remaining");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].slot_name, "slot_b");

        assert!(store.delete_slot("slot_a").is_err());
        assert!(store.load_slot("slot_a").is_err());
    }

    #[test]
    fn blank_slot_name_and_missing_json_return_errors() {
        let store = SaveStore::open(":memory:").expect("in-memory sqlite");
        let state = GameState::new_starter_factory();
        assert!(store.save_slot("   ", &state).is_err());
        assert!(store.load_slot("").is_err());
        assert!(store.delete_slot("\t ").is_err());

        let missing = std::env::temp_dir().join("swarmforge_nonexistent_test_file_12345.json");
        assert!(import_json_file(&missing).is_err());

        let bad_dir = std::env::temp_dir().join("swarmforge_nonexistent_dir_xyz_99999").join("out.json");
        assert!(export_json_file(&bad_dir, &state).is_err());
    }
}
