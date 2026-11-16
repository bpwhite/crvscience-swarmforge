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

use crate::builder_ai::find_procedural_bay_slot;
use crate::model::{
    DeliveryContract, FactoryModule, FloorId, GameState, ModuleKind, Robot, RobotKind, RobotTask,
    PRODUCT_CATALOG,
};
use crate::navigation::{module_dock_point, step_robot_with_collision};
use crate::swarm_tech::compute_multipliers;

pub const MAX_MAIN_FLOOR_HALF_EXTENT: i32 = 13; // Up to 26x26 Megafactory slab!

pub fn tick_support_floors(state: &mut GameState, dt: f32) {
    let mult = compute_multipliers(state);

    // 1. Basement Infrastructure
    let mut supply_kw = 0.0f32;
    let mut demand_kw = 0.0f32;
    let mut chiller_count = 0usize;
    let mut pipe_manifolds = 0usize;
    let mut silo_count = 0usize;

    for m in &mut state.modules {
        let draw = m.kind.power_draw_kw();
        if draw < 0.0 {
            supply_kw += (-draw) * mult.power_capacity_mult * m.condition.max(0.4);
        } else {
            demand_kw += draw;
        }
        match m.kind {
            ModuleKind::PlumbingChiller => chiller_count += 1,
            ModuleKind::HydraulicPipingManifold => pipe_manifolds += 1,
            ModuleKind::StorageSilo => silo_count += 1,
            _ => {}
        }
        m.condition = (m.condition - 0.0008 * dt).max(0.45);
    }
    demand_kw += state.robots.len() as f32 * 0.65;
    state.power_supply_kw = supply_kw.max(25.0);
    state.power_demand_kw = demand_kw;

    let target_psi = 72.0 + (chiller_count as f32 * 14.0) + (pipe_manifolds as f32 * 18.0);
    state.coolant_pressure_psi += (target_psi - state.coolant_pressure_psi) * (dt * 0.8).min(1.0);

    let hedge_servers = state
        .modules
        .iter()
        .filter(|m| m.kind == ModuleKind::SupplyHedgeServer)
        .count() as f32;
    let max_stock = 80.0 + silo_count as f32 * 90.0;
    let refill_rate = 1.6 + silo_count as f32 * 1.4 + hedge_servers * 1.8;
    state.raw_material_stock = (state.raw_material_stock + refill_rate * dt).min(max_stock);

    // Maintenance Bots (solid collision & exterior docking)
    let repair_boost = if state.has_tech("robo_holo_litho") { 1.6 } else { 1.0 };
    let maint_indices: Vec<usize> = state
        .robots
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RobotKind::Maintenance)
        .map(|(i, _)| i)
        .collect();

    for idx in maint_indices {
        let speed = state.robots[idx].kind.base_speed() * mult.move_speed;
        let task = state.robots[idx].task.clone();
        match task {
            RobotTask::Idle => {
                let bot_id = state.robots[idx].id;
                let pick = state
                    .modules
                    .iter()
                    .filter(|m| m.floor == FloorId::Basement)
                    .min_by(|a, b| a.condition.partial_cmp(&b.condition).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|m| (m.id, module_dock_point(m, bot_id, &state.modules, &state.robots, 5)));
                if let Some((mid, (dx, dz))) = pick {
                    state.robots[idx].target_x = dx;
                    state.robots[idx].target_z = dz;
                    state.robots[idx].task = RobotTask::Operating {
                        module_id: mid,
                        timer: 2.0,
                    };
                }
            }
            RobotTask::Operating {
                module_id,
                mut timer,
            } => {
                if step_robot_with_collision(state, idx, speed, dt) {
                    timer -= dt;
                    if let Some(m) = state.modules.iter_mut().find(|m| m.id == module_id) {
                        m.condition = (m.condition + 0.14 * repair_boost * dt).min(1.0);
                    }
                    for m in state.modules.iter_mut() {
                        m.condition = (m.condition + 0.008 * repair_boost * dt).min(1.0);
                    }
                    if timer <= 0.0 {
                        state.robots[idx].task = RobotTask::Idle;
                        state.robots[idx].tasks_completed += 1;
                    } else {
                        state.robots[idx].task = RobotTask::Operating { module_id, timer };
                    }
                }
            }
            _ => state.robots[idx].task = RobotTask::Idle,
        }
    }

    // 2. Floor 2: Tech & Research Labs
    let f2_count = state
        .modules
        .iter()
        .filter(|m| m.floor == FloorId::TechResearch)
        .count();
    state.research_points += (f2_count as f64) * 0.18 * (mult.research_rate as f64) * (dt as f64);

    let res_indices: Vec<usize> = state
        .robots
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RobotKind::Researcher)
        .map(|(i, _)| i)
        .collect();

    let mut active_rp_gain = 0.0f64;
    for (ord, idx) in res_indices.into_iter().enumerate() {
        let speed = state.robots[idx].kind.base_speed() * mult.move_speed;
        let task = state.robots[idx].task.clone();
        match task {
            RobotTask::Idle => {
                let bot_id = state.robots[idx].id;
                let completed = state.robots[idx].tasks_completed as usize;
                let f2_mods: Vec<&FactoryModule> = state
                    .modules
                    .iter()
                    .filter(|m| m.floor == FloorId::TechResearch)
                    .collect();
                if !f2_mods.is_empty() {
                    let m = f2_mods[(completed + ord) % f2_mods.len()];
                    let mid = m.id;
                    let (dx, dz) = module_dock_point(m, bot_id, &state.modules, &state.robots, 5);
                    state.robots[idx].target_x = dx;
                    state.robots[idx].target_z = dz;
                    state.robots[idx].task = RobotTask::Operating {
                        module_id: mid,
                        timer: 2.5,
                    };
                }
            }
            RobotTask::Operating {
                module_id,
                mut timer,
            } => {
                if step_robot_with_collision(state, idx, speed, dt) {
                    timer -= dt;
                    let tier_mult = 1.0 + (state.robots[idx].tier.saturating_sub(1) as f64) * 0.35;
                    active_rp_gain += 0.95 * (mult.research_rate as f64) * tier_mult * (dt as f64);
                    if timer <= 0.0 {
                        state.robots[idx].task = RobotTask::Idle;
                        state.robots[idx].tasks_completed += 1;
                    } else {
                        state.robots[idx].task = RobotTask::Operating { module_id, timer };
                    }
                }
            }
            _ => state.robots[idx].task = RobotTask::Idle,
        }
    }
    state.research_points += active_rp_gain;

    // 3. Floor 3: Robotic Administration
    let admin_indices: Vec<usize> = state
        .robots
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RobotKind::Administrator)
        .map(|(i, _)| i)
        .collect();
    let admin_bots = admin_indices.len();

    for (ord, idx) in admin_indices.into_iter().enumerate() {
        let speed = state.robots[idx].kind.base_speed() * mult.move_speed;
        let task = state.robots[idx].task.clone();
        match task {
            RobotTask::Idle => {
                let bot_id = state.robots[idx].id;
                let completed = state.robots[idx].tasks_completed as usize;
                let f3_mods: Vec<&FactoryModule> = state
                    .modules
                    .iter()
                    .filter(|m| m.floor == FloorId::Administration)
                    .collect();
                if !f3_mods.is_empty() {
                    let m = f3_mods[(completed + ord) % f3_mods.len()];
                    let mid = m.id;
                    let (dx, dz) = module_dock_point(m, bot_id, &state.modules, &state.robots, 4);
                    state.robots[idx].target_x = dx;
                    state.robots[idx].target_z = dz;
                    state.robots[idx].task = RobotTask::Operating {
                        module_id: mid,
                        timer: 3.0,
                    };
                }
            }
            RobotTask::Operating {
                module_id,
                mut timer,
            } => {
                if step_robot_with_collision(state, idx, speed, dt) {
                    timer -= dt;
                    if timer <= 0.0 {
                        state.robots[idx].task = RobotTask::Idle;
                        state.robots[idx].tasks_completed += 1;
                    } else {
                        state.robots[idx].task = RobotTask::Operating { module_id, timer };
                    }
                }
            }
            _ => state.robots[idx].task = RobotTask::Idle,
        }
    }

    let contract_desks = state
        .modules
        .iter()
        .filter(|m| m.kind == ModuleKind::ContractDispatchTerminal)
        .count() as f32;
    state.admin_margin_multiplier =
        (1.0 + admin_bots as f32 * 0.14 + contract_desks * 0.10) * mult.sale_value;

    if state.contracts.iter().all(|c| c.completed)
        && (admin_bots > 0 || state.has_tech("robo_floor3_admin"))
    {
        let next_cid = state.contracts.len() as u32 + 1;
        let tier = state.active_product_tier;
        let bp = &PRODUCT_CATALOG[(tier.saturating_sub(1) as usize).min(PRODUCT_CATALOG.len() - 1)];
        let units = 20 + next_cid * 10;
        let bonus_credits = (units as f64) * bp.base_value * 0.55;
        let bonus_rp = 25.0 + (next_cid as f64) * 15.0;
        state.contracts.push(DeliveryContract {
            id: next_cid,
            title: format!("Robotic B2B Batch #{next_cid}: {} ({units}x)", bp.name),
            product_tier: tier,
            required_units: units,
            delivered_units: 0,
            bonus_credits,
            bonus_rp,
            completed: false,
        });
        state.push_log(format!(
            "Floor 3 Admin Bots signed Contract #{next_cid} for {units}x {}!",
            bp.name
        ));
    }
}

pub fn purchase_robot(state: &mut GameState, kind: RobotKind) -> Result<u32, String> {
    let count = state.robots.iter().filter(|r| r.kind == kind).count() as u32;
    let cost = robot_current_cost(kind, count);
    if state.credits < cost {
        return Err(format!("Need ₡{cost:.0} credits."));
    }
    state.credits -= cost;
    let id = state.alloc_id();
    let floor = kind.home_floor();
    let angle = (id as f32) * 1.15;
    let rad = 1.6 + ((id % 4) as f32) * 0.65;
    let half = if floor == FloorId::MainFloor { state.main_floor_half_extent } else { 4 };
    let (sx, sz) = crate::navigation::find_safe_open_point(
        &state.modules, floor, half, angle.cos() * rad, angle.sin() * rad, 0.54
    );
    state.robots.push(Robot {
        id,
        kind,
        floor,
        x: sx,
        z: sz,
        target_x: sx,
        target_z: sz,
        heading: angle,
        tier: 1,
        energy: 1.0,
        integrity: 1.0,
        morale_boost_timer: 0.0,
        task: RobotTask::Idle,
        tasks_completed: 0,
    });
    crate::navigation::resolve_floor_collisions(state, floor);
    state.push_log(format!(
        "Commissioned {} #{} on {}.",
        kind.label(),
        id,
        floor.code()
    ));
    Ok(id)
}

pub fn robot_current_cost(kind: RobotKind, existing_of_kind: u32) -> f64 {
    kind.base_cost() * 1.18_f64.powi(existing_of_kind as i32)
}

pub fn purchase_module(state: &mut GameState, kind: ModuleKind) -> Result<u32, String> {
    let count = state.modules.iter().filter(|m| m.kind == kind).count() as u32;
    let cost = module_current_cost(kind, count);
    if state.credits < cost {
        return Err(format!("Need ₡{cost:.0} credits."));
    }
    let floor = kind.floor();
    let Some((gx, gz)) = find_procedural_bay_slot(state, floor) else {
        return Err(if floor == FloorId::MainFloor {
            "Minimum 2-bay module clearance reached on F1 — click '+ Expand F1 Floor'!".to_string()
        } else {
            format!("All spaced bays occupied on {}!", floor.code())
        });
    };

    state.credits -= cost;
    let id = state.alloc_id();
    state.modules.push(FactoryModule::new(id, kind, floor, gx, gz));
    crate::navigation::resolve_floor_collisions(state, floor);
    state.push_log(format!(
        "Installed {} at {} bay ({gx}, {gz}) with full corridor clearance.",
        kind.label(),
        floor.code()
    ));
    Ok(id)
}

pub fn module_current_cost(kind: ModuleKind, existing_of_kind: u32) -> f64 {
    kind.base_cost() * 1.20_f64.powi(existing_of_kind as i32)
}

pub fn expansion_cost(state: &GameState) -> Option<f64> {
    if state.main_floor_half_extent >= MAX_MAIN_FLOOR_HALF_EXTENT {
        return None;
    }
    let step = (state.main_floor_half_extent - 5).max(0);
    let base = 110.0 * 1.62_f64.powi(step);
    let discount = if state.has_tech("robo_bay_expansion") {
        0.70
    } else {
        1.0
    };
    Some(base * discount)
}

pub fn expand_main_floor(state: &mut GameState) -> Result<(), String> {
    let Some(cost) = expansion_cost(state) else {
        return Err("Main Factory Floor already at maximum 26x26 Megafactory span.".to_string());
    };
    if state.credits < cost {
        return Err(format!("Need ₡{cost:.0} credits to expand Floor 1."));
    }
    state.credits -= cost;
    state.main_floor_half_extent += 1;
    let span = state.main_floor_half_extent * 2;
    state.push_log(format!(
        "Expanded Floor 1 Main Slab to {span}x{span} bays!"
    ));
    Ok(())
}
