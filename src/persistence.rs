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
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleUtilizationRow {
    pub id: u32,
    pub kind: String,
    pub floor: String,
    pub input_buffer: u32,
    pub output_buffer: u32,
    pub work_progress: f32,
    pub condition: f32,
    pub power_draw_kw: f32,
    pub utilization_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryKpiDigest {
    pub schema_version: String,
    pub version: String,
    pub elapsed_secs: f64,
    pub tick_count: u64,
    pub total_products_shipped: u64,
    pub throughput_units_per_min: f64,
    pub credits: f64,
    pub total_revenue: f64,
    pub research_points: f64,
    pub layout_efficiency: f32,
    pub swarm_coherence: f32,
    pub robot_count: usize,
    pub active_robot_ratio: f32,
    pub mean_robot_energy: f32,
    pub mean_robot_integrity: f32,
    pub tasks_completed_total: u64,
    pub tasks_per_robot: f64,
    pub module_count: usize,
    pub mean_module_utilization: f32,
    pub avg_module_condition: f32,
    pub bottleneck_module_id: Option<u32>,
    pub bottleneck_module_label: String,
    pub bottleneck_backlog_units: u32,
    pub power_supply_kw: f32,
    pub power_demand_kw: f32,
    pub power_headroom_kw: f32,
    pub power_utilization_pct: f32,
    pub coolant_pressure_psi: f32,
    pub energy_efficiency_units_per_kwh: f64,
    pub modules: Vec<ModuleUtilizationRow>,
}

fn csv_escape_kpi(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

pub fn build_factory_kpi_digest(state: &GameState) -> FactoryKpiDigest {
    let elapsed_min = (state.elapsed_secs / 60.0).max(1.0 / 60.0);
    let throughput_units_per_min = (state.total_products_shipped as f64) / elapsed_min;

    let robot_count = state.robots.len();
    let active_robots = state
        .robots
        .iter()
        .filter(|r| !matches!(r.task, crate::model::RobotTask::Idle))
        .count();
    let active_robot_ratio = if robot_count == 0 {
        0.0
    } else {
        active_robots as f32 / robot_count as f32
    };
    let mean_robot_energy = if robot_count == 0 {
        0.0
    } else {
        state.robots.iter().map(|r| r.energy).sum::<f32>() / robot_count as f32
    };
    let mean_robot_integrity = if robot_count == 0 {
        0.0
    } else {
        state.robots.iter().map(|r| r.integrity).sum::<f32>() / robot_count as f32
    };
    let tasks_completed_total: u64 = state.robots.iter().map(|r| r.tasks_completed).sum();
    let tasks_per_robot = if robot_count == 0 {
        0.0
    } else {
        tasks_completed_total as f64 / robot_count as f64
    };

    let mut module_rows = Vec::with_capacity(state.modules.len());
    let mut bottleneck_id: Option<u32> = None;
    let mut bottleneck_label = "None".to_string();
    let mut bottleneck_backlog = 0u32;
    let mut util_sum = 0.0f32;
    let mut cond_sum = 0.0f32;

    for m in &state.modules {
        let backlog = m.input_buffer + m.output_buffer;
        let util = ((backlog as f32 / 8.0) + m.work_progress).clamp(0.0, 1.0);
        util_sum += util;
        cond_sum += m.condition;
        if bottleneck_id.is_none() || backlog > bottleneck_backlog {
            bottleneck_id = Some(m.id);
            bottleneck_label = m.kind.label().to_string();
            bottleneck_backlog = backlog;
        }
        module_rows.push(ModuleUtilizationRow {
            id: m.id,
            kind: m.kind.label().to_string(),
            floor: m.floor.code().to_string(),
            input_buffer: m.input_buffer,
            output_buffer: m.output_buffer,
            work_progress: m.work_progress,
            condition: m.condition,
            power_draw_kw: m.kind.power_draw_kw(),
            utilization_score: util,
        });
    }

    let mod_n = state.modules.len().max(1) as f32;
    let mean_module_utilization = if state.modules.is_empty() {
        0.0
    } else {
        util_sum / mod_n
    };
    let avg_module_condition = if state.modules.is_empty() {
        1.0
    } else {
        cond_sum / mod_n
    };
    let power_headroom_kw = state.power_supply_kw - state.power_demand_kw;
    let power_utilization_pct = if state.power_supply_kw > 0.0 {
        (state.power_demand_kw / state.power_supply_kw) * 100.0
    } else {
        0.0
    };
    let kwh_consumed =
        (state.power_demand_kw.max(0.1) as f64) * (state.elapsed_secs / 3600.0).max(1.0 / 3600.0);
    let energy_efficiency_units_per_kwh = (state.total_products_shipped as f64) / kwh_consumed;

    FactoryKpiDigest {
        schema_version: "1.0".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        elapsed_secs: state.elapsed_secs,
        tick_count: state.tick_count,
        total_products_shipped: state.total_products_shipped,
        throughput_units_per_min,
        credits: state.credits,
        total_revenue: state.total_revenue,
        research_points: state.research_points,
        layout_efficiency: state.layout_efficiency,
        swarm_coherence: state.swarm_coherence,
        robot_count,
        active_robot_ratio,
        mean_robot_energy,
        mean_robot_integrity,
        tasks_completed_total,
        tasks_per_robot,
        module_count: state.modules.len(),
        mean_module_utilization,
        avg_module_condition,
        bottleneck_module_id: bottleneck_id,
        bottleneck_module_label: bottleneck_label,
        bottleneck_backlog_units: bottleneck_backlog,
        power_supply_kw: state.power_supply_kw,
        power_demand_kw: state.power_demand_kw,
        power_headroom_kw,
        power_utilization_pct,
        coolant_pressure_psi: state.coolant_pressure_psi,
        energy_efficiency_units_per_kwh,
        modules: module_rows,
    }
}

pub fn format_kpi_digest_csv(digest: &FactoryKpiDigest) -> String {
    let mut lines = Vec::with_capacity(2 + digest.modules.len());
    lines.push(
        "scope,id,label,floor,throughput_units_per_min,utilization_score,backlog_units,condition,power_kw,energy_efficiency_units_per_kwh,active_robot_ratio,swarm_coherence"
            .to_string(),
    );
    lines.push(format!(
        "factory,0,Factory Summary,ALL,{:.4},{:.4},{},{:.4},{:.2},{:.4},{:.4},{:.3}",
        digest.throughput_units_per_min,
        digest.mean_module_utilization,
        digest.bottleneck_backlog_units,
        digest.avg_module_condition,
        digest.power_demand_kw,
        digest.energy_efficiency_units_per_kwh,
        digest.active_robot_ratio,
        digest.swarm_coherence,
    ));
    for m in &digest.modules {
        lines.push(format!(
            "module,{},{},{},{:.4},{:.4},{},{:.4},{:.2},{:.4},{:.4},{:.3}",
            m.id,
            csv_escape_kpi(&m.kind),
            csv_escape_kpi(&m.floor),
            digest.throughput_units_per_min,
            m.utilization_score,
            m.input_buffer + m.output_buffer,
            m.condition,
            m.power_draw_kw,
            digest.energy_efficiency_units_per_kwh,
            digest.active_robot_ratio,
            digest.swarm_coherence,
        ));
    }
    lines.join("\n") + "\n"
}

pub fn export_kpi_digest_json(path: impl AsRef<Path>, state: &GameState) -> Result<(), String> {
    let digest = build_factory_kpi_digest(state);
    let json = serde_json::to_string_pretty(&digest)
        .map_err(|e| format!("KPI digest JSON serialize error: {e}"))?;
    fs::write(path, json).map_err(|e| format!("KPI digest JSON write error: {e}"))
}

pub fn export_kpi_digest_csv(path: impl AsRef<Path>, state: &GameState) -> Result<(), String> {
    let digest = build_factory_kpi_digest(state);
    let csv = format_kpi_digest_csv(&digest);
    fs::write(path, csv).map_err(|e| format!("KPI digest CSV write error: {e}"))
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

    #[test]
    fn factory_kpi_telemetry_digest_json_and_csv_capture_throughput_bottlenecks_and_energy() {
        let mut state = GameState::new_multi_floor_factory();
        for _ in 0..80 {
            crate::assembly::tick_factory(&mut state, 0.1);
        }
        let digest = build_factory_kpi_digest(&state);
        assert_eq!(digest.module_count, state.modules.len());
        assert_eq!(digest.robot_count, state.robots.len());
        assert!(digest.power_supply_kw > 0.0);
        assert!(digest.bottleneck_module_id.is_some());
        assert!(digest.mean_robot_energy > 0.0);
        assert!(digest.avg_module_condition > 0.0);

        let tmp = std::env::temp_dir().join(format!("swarmforge_kpi_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).expect("mkdir");
        let json_p = tmp.join("kpi.json");
        let csv_p = tmp.join("kpi.csv");
        export_kpi_digest_json(&json_p, &state).expect("export kpi json");
        export_kpi_digest_csv(&csv_p, &state).expect("export kpi csv");

        let parsed: FactoryKpiDigest =
            serde_json::from_str(&fs::read_to_string(&json_p).expect("read json")).expect("parse");
        assert_eq!(parsed.module_count, digest.module_count);
        let csv_raw = fs::read_to_string(&csv_p).expect("read csv");
        // 1 header + 1 factory summary + N modules
        assert_eq!(csv_raw.lines().count(), 2 + state.modules.len());
        assert!(csv_raw.contains("energy_efficiency_units_per_kwh"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
