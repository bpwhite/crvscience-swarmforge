#![forbid(unsafe_code)]
//! Industrial Disaster, Fire Suppression, Routine Bot Maintenance, CheerBot Morale,
//! Outbound Delivery Drone Sorties, and Progressive Warehouse Flare Calculation.

use crate::model::{
    ActiveDisaster, DisasterKind, FloorId, GameState, ModuleKind, RobotKind, RobotTask,
};
use crate::navigation::{module_dock_point, step_robot_with_collision};
use crate::swarm_tech::compute_multipliers;
use rand::Rng;

/// Coverage radius (in world units) of each `FireSuppressionTower`.
pub const SUPPRESSION_RADIUS: f32 = 10.5;

/// Morale aura radius (in world units) of each cheering `MotivationDrone`.
pub const CHEER_AURA_RADIUS: f32 = 8.5;

/// Computes the non-obstructing Warehouse Flare Tier (`1..=6`) from cumulative factory growth.
pub fn compute_warehouse_flare_tier(state: &GameState) -> u8 {
    let score = (state.total_products_shipped as usize / 8)
        + state.unlocked_techs.len() * 2
        + state.robots.len() / 2
        + (state.main_floor_half_extent.saturating_sub(5) as usize) * 2;
    match score {
        0..=2 => 1,
        3..=6 => 2,
        7..=13 => 3,
        14..=22 => 4,
        23..=35 => 5,
        _ => 6,
    }
}

/// Ticks disasters, Fire Suppression Towers, Repair Bots, Bot Repair Stations,
/// Motivation Drones ("cheer" bursts), and periodic Delivery Dock cargo drone fly-offs.
pub fn tick_disasters_repair_and_drones(state: &mut GameState, dt: f32) {
    state.warehouse_flare_tier = compute_warehouse_flare_tier(state);

    // 1. Periodic Delivery Dock Drone Fly-Off Sortie Animation
    for m in state.modules.iter_mut() {
        if m.kind == ModuleKind::DeliveryDock && !m.being_relocated {
            if m.drone_sortie_progress > 0.0 {
                m.drone_sortie_progress += dt * 0.24; // ~4.2s full climb -> fly-off -> return -> land cycle
                if m.drone_sortie_progress >= 1.0 {
                    m.drone_sortie_progress = 0.0;
                }
            }
        }
    }

    // 2. Stochastic Disaster Spawning (scaled gently so 1-bot starter factories aren't overwhelmed)
    if state.elapsed_secs > 22.0 && state.disasters.len() < 3 && state.tick_count % 25 == 0 {
        let overload_ratio = (state.power_demand_kw / state.power_supply_kw.max(1.0)).max(0.5);
        let base_chance = 0.012 * overload_ratio * (1.0 + state.modules.len() as f32 * 0.025);
        let mut rng = rand::thread_rng();
        if rng.gen::<f32>() < base_chance {
            spawn_random_disaster(state);
        }
    }

    // 3. Apply Active Disaster Effects & Automatic FireSuppressionTower Cryo-Foam Streams!
    let towers: Vec<(f32, f32)> = state
        .modules
        .iter()
        .filter(|m| {
            m.floor == FloorId::MainFloor
                && m.kind == ModuleKind::FireSuppressionTower
                && !m.being_relocated
        })
        .map(|m| (m.visual_x, m.visual_z))
        .collect();

    let mut resolved_logs = Vec::new();
    for d in state.disasters.iter_mut() {
        // Find affected module position & apply wear/damage
        if let Some(m) = state.modules.iter_mut().find(|m| m.id == d.module_id) {
            d.x = m.visual_x;
            d.z = m.visual_z;
            m.condition = (m.condition - 0.022 * d.severity * dt).max(0.20);

            // Check if any Fire Suppression Tower on Floor 1 is in range!
            let mut tower_suppression = 0.0f32;
            if m.floor == FloorId::MainFloor {
                for &(tx, tz) in &towers {
                    if (tx - m.visual_x).hypot(tz - m.visual_z) <= SUPPRESSION_RADIUS {
                        tower_suppression += 0.34;
                    }
                }
            }
            d.suppressed_by_tower = tower_suppression > 0.0;
            if tower_suppression > 0.0 {
                d.severity -= tower_suppression * dt;
            } else {
                // Slow natural self-burnout or slight growth up to 1.0
                d.severity = (d.severity - 0.015 * dt).clamp(0.0, 1.0);
            }
            if d.severity <= 0.02 {
                resolved_logs.push(format!(
                    "{} cleared at {}.",
                    d.kind.label(),
                    m.kind.label()
                ));
            }
        } else {
            d.severity = 0.0;
        }
    }
    state.disasters.retain(|d| d.severity > 0.02);
    for msg in resolved_logs {
        state.disasters_resolved += 1;
        state.push_log(msg);
    }

    // 4. Routine Robot Wear + Autonomous Pit-Stop at `BotRepairStation`
    let has_repair_station = state
        .modules
        .iter()
        .any(|m| m.floor == FloorId::MainFloor && m.kind == ModuleKind::BotRepairStation && !m.being_relocated);

    for r in state.robots.iter_mut() {
        if matches!(r.kind, RobotKind::MotivationDrone | RobotKind::RepairBot) {
            r.integrity = (r.integrity + 0.03 * dt).min(1.0);
            continue;
        }
        // Gentle routine wear while working; never drops below 0.22
        let wear_rate = if matches!(r.task, RobotTask::Idle) {
            0.0015
        } else {
            0.0065
        };
        r.integrity = (r.integrity - wear_rate * dt).max(0.22);
        r.morale_boost_timer = (r.morale_boost_timer - dt).max(0.0);
    }

    // Route worn Idle Floor 1 bots (`integrity < 0.48`) to a BotRepairStation (or slow tune-up at ProtoBench)
    let mult = compute_multipliers(state);
    for r_idx in 0..state.robots.len() {
        if state.robots[r_idx].floor != FloorId::MainFloor
            || matches!(
                state.robots[r_idx].kind,
                RobotKind::MotivationDrone | RobotKind::RepairBot | RobotKind::Builder
            )
        {
            continue;
        }
        match state.robots[r_idx].task.clone() {
            RobotTask::Idle if state.robots[r_idx].integrity < 0.48 => {
                let bot_id = state.robots[r_idx].id;
                let station = state
                    .modules
                    .iter()
                    .filter(|m| {
                        m.floor == FloorId::MainFloor
                            && !m.being_relocated
                            && (m.kind == ModuleKind::BotRepairStation
                                || (!has_repair_station && m.kind == ModuleKind::ProtoBench))
                    })
                    .min_by_key(|m| {
                        ((m.visual_x - state.robots[r_idx].x).powi(2)
                            + (m.visual_z - state.robots[r_idx].z).powi(2))
                            as i32
                    })
                    .map(|m| {
                        (
                            m.id,
                            module_dock_point(
                                m,
                                bot_id,
                                &state.modules,
                                &state.robots,
                                state.main_floor_half_extent,
                            ),
                        )
                    });
                if let Some((sid, (dx, dz))) = station {
                    state.robots[r_idx].target_x = dx;
                    state.robots[r_idx].target_z = dz;
                    state.robots[r_idx].task = RobotTask::SeekingMaintenance { station_id: sid };
                }
            }
            RobotTask::SeekingMaintenance { station_id } => {
                let spd = state.robots[r_idx].kind.base_speed() * mult.move_speed;
                if step_robot_with_collision(state, r_idx, spd, dt) {
                    let rate = if has_repair_station { 0.48 } else { 0.22 };
                    state.robots[r_idx].integrity =
                        (state.robots[r_idx].integrity + rate * dt).min(1.0);
                    if state.robots[r_idx].integrity >= 0.99 {
                        state.robots[r_idx].morale_boost_timer = 8.0;
                        state.robots[r_idx].task = RobotTask::Idle;
                        state.robots[r_idx].tasks_completed += 1;
                    } else {
                        state.robots[r_idx].task = RobotTask::SeekingMaintenance { station_id };
                    }
                }
            }
            _ => {}
        }
    }

    // 5. Tick `RepairBot` Fleet (Priority 1: Extinguish Disasters; Priority 2: Mobile Robot & Fab Repair!)
    let repair_indices: Vec<usize> = state
        .robots
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RobotKind::RepairBot)
        .map(|(i, _)| i)
        .collect();

    for idx in repair_indices {
        let spd = state.robots[idx].kind.base_speed() * mult.move_speed;
        let bot_id = state.robots[idx].id;

        // Priority 1: Active Floor 1 disaster!
        let urgent_mod = state
            .disasters
            .iter()
            .filter(|d| d.floor == FloorId::MainFloor)
            .max_by(|a, b| {
                a.severity
                    .partial_cmp(&b.severity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|d| d.module_id);

        if let Some(d_mid) = urgent_mod {
            if let Some((dx, dz)) = state
                .modules
                .iter()
                .find(|m| m.id == d_mid)
                .map(|m| {
                    module_dock_point(
                        m,
                        bot_id,
                        &state.modules,
                        &state.robots,
                        state.main_floor_half_extent,
                    )
                })
            {
                state.robots[idx].target_x = dx;
                state.robots[idx].target_z = dz;
                state.robots[idx].task = RobotTask::Operating {
                    module_id: d_mid,
                    timer: 1.5,
                };
                if step_robot_with_collision(state, idx, spd * 1.2, dt) {
                    for d in state.disasters.iter_mut() {
                        if d.module_id == d_mid {
                            d.severity = (d.severity - 0.52 * dt).max(0.0);
                        }
                    }
                    if let Some(m) = state.modules.iter_mut().find(|m| m.id == d_mid) {
                        m.condition = (m.condition + 0.25 * dt).min(1.0);
                    }
                }
                continue;
            }
        }

        // Priority 2: Mobile repair of nearby worn robots & worn Floor 1 modules
        let rx = state.robots[idx].x;
        let rz = state.robots[idx].z;
        for other in state.robots.iter_mut() {
            if other.id != bot_id
                && other.floor == FloorId::MainFloor
                && (other.x - rx).hypot(other.z - rz) <= 3.2
            {
                other.integrity = (other.integrity + 0.28 * dt).min(1.0);
            }
        }

        let need_bot = state
            .robots
            .iter()
            .filter(|o| o.id != bot_id && o.floor == FloorId::MainFloor && o.integrity < 0.78)
            .min_by(|a, b| {
                a.integrity
                    .partial_cmp(&b.integrity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|o| (o.x, o.z));

        if let Some((bx, bz)) = need_bot {
            let (sx, sz) = crate::navigation::find_safe_open_point(
                &state.modules,
                FloorId::MainFloor,
                state.main_floor_half_extent,
                bx + 0.9,
                bz,
                0.52,
            );
            state.robots[idx].target_x = sx;
            state.robots[idx].target_z = sz;
            let _ = step_robot_with_collision(state, idx, spd, dt);
        } else if let Some((mid, (dx, dz))) = state
            .modules
            .iter()
            .filter(|m| m.floor == FloorId::MainFloor && !m.being_relocated && m.condition < 0.92)
            .min_by(|a, b| {
                a.condition
                    .partial_cmp(&b.condition)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|m| {
                (
                    m.id,
                    module_dock_point(
                        m,
                        bot_id,
                        &state.modules,
                        &state.robots,
                        state.main_floor_half_extent,
                    ),
                )
            })
        {
            state.robots[idx].target_x = dx;
            state.robots[idx].target_z = dz;
            if step_robot_with_collision(state, idx, spd, dt) {
                if let Some(m) = state.modules.iter_mut().find(|m| m.id == mid) {
                    m.condition = (m.condition + 0.22 * dt).min(1.0);
                }
                state.robots[idx].tasks_completed += 1;
            }
        } else {
            let d2 = (state.robots[idx].target_x - rx).powi(2)
                + (state.robots[idx].target_z - rz).powi(2);
            if d2 < 0.65 {
                let ang = state.elapsed_secs as f32 * 0.6 + bot_id as f32;
                let rad = state.main_floor_half_extent as f32 * 1.3;
                let (px, pz) = crate::navigation::find_safe_open_point(
                    &state.modules,
                    FloorId::MainFloor,
                    state.main_floor_half_extent,
                    ang.cos() * rad,
                    ang.sin() * rad,
                    0.52,
                );
                state.robots[idx].target_x = px;
                state.robots[idx].target_z = pz;
            }
            let _ = step_robot_with_collision(state, idx, spd * 0.75, dt);
        }
    }

    // 6. Tick Airborne `MotivationDrone` Units (Overhead non-obstructing flight + periodic "Cheer!" bursts!)
    let cheer_indices: Vec<usize> = state
        .robots
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RobotKind::MotivationDrone)
        .map(|(i, _)| i)
        .collect();

    const CHEER_CHANTS: [&str; 5] = [
        "GO SWARM! +20% CADENCE!",
        "ZERO DEFECTS! WOO!",
        "SERVO POWER MAXIMUM!",
        "OPTIMIZE & SHINE!",
        "BEEP-BOOP HUSTLE!",
    ];

    for idx in cheer_indices {
        let drone_id = state.robots[idx].id;
        // Pick a ground worker on Floor 1 to hover over and cheer!
        let ground_targets: Vec<(f32, f32)> = state
            .robots
            .iter()
            .filter(|o| o.floor == FloorId::MainFloor && o.kind != RobotKind::MotivationDrone)
            .map(|o| (o.x, o.z))
            .collect();

        if !ground_targets.is_empty() {
            let pick = ground_targets
                [((state.elapsed_secs as usize / 4) + drone_id as usize) % ground_targets.len()];
            state.robots[idx].target_x = pick.0;
            state.robots[idx].target_z = pick.1;
        }
        // Smooth airborne glide (zero ground collision!)
        let dx = state.robots[idx].target_x - state.robots[idx].x;
        let dz = state.robots[idx].target_z - state.robots[idx].z;
        let d = dx.hypot(dz);
        if d > 0.08 {
            let step = (3.4 * dt).min(d);
            state.robots[idx].x += (dx / d) * step;
            state.robots[idx].z += (dz / d) * step;
            state.robots[idx].heading = dz.atan2(dx);
        }

        let cx = state.robots[idx].x;
        let cz = state.robots[idx].z;
        state.robots[idx].morale_boost_timer -= dt;
        if state.robots[idx].morale_boost_timer <= 0.0 {
            state.robots[idx].morale_boost_timer = 5.5;
            state.robots[idx].tasks_completed += 1;
            let chant = CHEER_CHANTS
                [(state.robots[idx].tasks_completed as usize + drone_id as usize) % CHEER_CHANTS.len()];
            state.last_cheer_banner = format!("CheerBot #{drone_id}: \"{chant}\"");
            // Buff all robots within CHEER_AURA_RADIUS!
            for r in state.robots.iter_mut() {
                if r.floor == FloorId::MainFloor
                    && r.kind != RobotKind::MotivationDrone
                    && (r.x - cx).hypot(r.z - cz) <= CHEER_AURA_RADIUS
                {
                    r.morale_boost_timer = 6.5;
                }
            }
        }
    }
}

pub fn spawn_random_disaster(state: &mut GameState) {
    let candidates: Vec<(u32, FloorId, f32, f32, ModuleKind)> = state
        .modules
        .iter()
        .filter(|m| {
            !m.being_relocated
                && !matches!(
                    m.kind,
                    ModuleKind::VerticalFreightLift
                        | ModuleKind::FireSuppressionTower
                        | ModuleKind::BotRepairStation
                )
                && !state.disasters.iter().any(|d| d.module_id == m.id)
        })
        .map(|m| (m.id, m.floor, m.visual_x, m.visual_z, m.kind))
        .collect();

    if candidates.is_empty() {
        return;
    }
    let mut rng = rand::thread_rng();
    let (mid, floor, x, z, mkind) = candidates[rng.gen_range(0..candidates.len())];
    let kind = if state.power_demand_kw > state.power_supply_kw && rng.gen_bool(0.55) {
        DisasterKind::GridOverload
    } else {
        match rng.gen_range(0..4) {
            0 => DisasterKind::ThermalFire,
            1 => DisasterKind::GridOverload,
            2 => DisasterKind::CoolantLeak,
            _ => DisasterKind::LogicGlitch,
        }
    };
    let id = state.alloc_id();
    state.disasters.push(ActiveDisaster {
        id,
        kind,
        floor,
        module_id: mid,
        x,
        z,
        severity: 0.78,
        suppressed_by_tower: false,
    });
    state.push_log(format!(
        "ALERT: {} erupted at {} on {}!",
        kind.label(),
        mkind.label(),
        floor.code()
    ));
}

/// Manual / automated emergency response button clearing or damping active disasters.
pub fn trigger_emergency_suppression(state: &mut GameState) {
    if state.disasters.is_empty() {
        return;
    }
    for d in state.disasters.iter_mut() {
        d.severity = (d.severity - 0.65).max(0.0);
    }
    state.disasters.retain(|d| d.severity > 0.02);
    state.disasters_resolved += 1;
    state.push_log("Emergency Halon & Breaker Override discharged across facility!");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FactoryModule, Robot};

    #[test]
    fn fire_suppression_tower_extinguishes_fire_and_cheer_drone_buffs_bots() {
        let mut state = GameState::new_starter_factory();
        let tid = state.alloc_id();
        state.modules.push(FactoryModule::new(
            tid,
            ModuleKind::FireSuppressionTower,
            FloorId::MainFloor,
            1,
            -3,
        ));
        let did = state.alloc_id();
        state.disasters.push(ActiveDisaster {
            id: did,
            kind: DisasterKind::ThermalFire,
            floor: FloorId::MainFloor,
            module_id: 2, // Starter ProtoBench at (3, -3) -> distance 4.8u <= 10.5u!
            x: 7.2,
            z: -7.2,
            severity: 0.80,
            suppressed_by_tower: false,
        });

        let cid = state.alloc_id();
        state.robots.push(Robot {
            id: cid,
            kind: RobotKind::MotivationDrone,
            floor: FloorId::MainFloor,
            x: 0.0,
            z: 0.0,
            target_x: 0.0,
            target_z: 0.0,
            heading: 0.0,
            tier: 1,
            energy: 1.0,
            integrity: 1.0,
            morale_boost_timer: 0.1,
            task: RobotTask::Idle,
            tasks_completed: 0,
        });

        for _ in 0..40 {
            tick_disasters_repair_and_drones(&mut state, 0.1);
        }

        assert!(
            state.disasters.is_empty(),
            "FireSuppressionTower should extinguish nearby fire!"
        );
        assert!(
            state.robots[0].morale_boost_timer > 0.0,
            "MotivationDrone should apply morale cadence buff to nearby worker!"
        );
    }
}
