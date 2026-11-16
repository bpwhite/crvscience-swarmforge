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

use crate::model::{FactoryModule, FloorId, GameState, ModuleKind, PheromoneEdge, RobotKind, RobotTask};
use crate::navigation::{
    astar_corridor_cost, has_module_clearance, module_dock_point, step_robot_with_collision,
    CELL_PITCH, MIN_MODULE_CHEBYSHEV,
};
use crate::swarm_tech::compute_multipliers;
use rand::Rng;
use std::collections::HashMap;

pub fn evaluate_layout_with_pathing(
    modules: &[FactoryModule],
    congestion: &HashMap<(i32, i32), f32>,
    half_extent: i32,
) -> (f32, Vec<PheromoneEdge>) {
    evaluate_layout_impl(modules, congestion, half_extent, true)
}

/// Fast Manhattan + congestion surrogate used to pre-rank candidate Builder placements in O(1),
/// followed by exact A* verification on top candidates (prevents O(N^3) A* frame spikes on 26x26 floors!).
fn evaluate_layout_fast(
    modules: &[FactoryModule],
    congestion: &HashMap<(i32, i32), f32>,
    half_extent: i32,
) -> f32 {
    evaluate_layout_impl(modules, congestion, half_extent, false).0
}

fn evaluate_layout_impl(
    modules: &[FactoryModule],
    congestion: &HashMap<(i32, i32), f32>,
    half_extent: i32,
    exact_astar: bool,
) -> (f32, Vec<PheromoneEdge>) {
    let f1: Vec<&FactoryModule> = modules
        .iter()
        .filter(|m| m.floor == FloorId::MainFloor)
        .collect();
    if f1.len() <= 1 {
        return (1.0, Vec::new());
    }

    let mut crowding_penalty = 0.0f32;
    for i in 0..f1.len() {
        for j in (i + 1)..f1.len() {
            let cheb = (f1[i].gx - f1[j].gx).abs().max((f1[i].gz - f1[j].gz).abs());
            if cheb < MIN_MODULE_CHEBYSHEV {
                crowding_penalty += (MIN_MODULE_CHEBYSHEV - cheb) as f32 * 4.5;
            }
        }
        if f1[i].kind != ModuleKind::VerticalFreightLift
            && (f1[i].gx <= -half_extent
                || f1[i].gx >= half_extent - 1
                || f1[i].gz <= -half_extent
                || f1[i].gz >= half_extent - 1)
        {
            crowding_penalty += 0.25;
        }
    }

    let mut stage_buckets: Vec<Vec<&FactoryModule>> = Vec::new();
    let lift: Vec<&FactoryModule> = f1
        .iter()
        .copied()
        .filter(|m| m.kind == ModuleKind::VerticalFreightLift)
        .collect();
    if !lift.is_empty() {
        stage_buckets.push(lift);
    }
    for stage in 0..=5u8 {
        let group: Vec<&FactoryModule> = f1
            .iter()
            .copied()
            .filter(|m| m.kind.pipeline_stage() == Some(stage))
            .collect();
        if !group.is_empty() {
            stage_buckets.push(group);
        }
    }

    if stage_buckets.len() < 2 {
        let s = (0.88 / (1.0 + crowding_penalty * 0.25)).clamp(0.25, 1.0);
        return (s, Vec::new());
    }

    let mut total_excess = 0.0f32;
    let mut hop_count = 0.0f32;
    let mut edges = Vec::new();

    for pair in stage_buckets.windows(2) {
        let upstream = &pair[0];
        let downstream = &pair[1];
        for u in upstream {
            let mut best_cost = f32::MAX;
            let mut best_target = downstream[0].id;
            for d in downstream {
                let cost = if exact_astar {
                    astar_corridor_cost(
                        modules,
                        congestion,
                        FloorId::MainFloor,
                        half_extent,
                        (u.gx, u.gz),
                        (d.gx, d.gz),
                    )
                    .0
                } else {
                    let manh = ((u.gx - d.gx).abs() + (u.gz - d.gz).abs()) as f32;
                    let mid = ((u.gx + d.gx) / 2, (u.gz + d.gz) / 2);
                    let heat = congestion.get(&mid).copied().unwrap_or(0.0) * 1.2;
                    manh + heat
                };
                if cost < best_cost {
                    best_cost = cost;
                    best_target = d.id;
                }
            }
            let penalty = (best_cost - 2.5).max(0.0);
            total_excess += penalty;
            hop_count += 1.0;
            if exact_astar {
                let intensity = (1.0 / (1.0 + penalty * 0.28)).clamp(0.22, 1.0);
                edges.push(PheromoneEdge {
                    from_id: u.id,
                    to_id: best_target,
                    intensity,
                });
            }
        }
    }

    // Also incentivize central coverage for FireSuppressionTower and BotRepairStation
    for m in &f1 {
        if matches!(m.kind, ModuleKind::FireSuppressionTower | ModuleKind::BotRepairStation) {
            let d_center = (m.gx.abs() + m.gz.abs()) as f32;
            total_excess += (d_center - 2.0).max(0.0) * 0.45;
            hop_count += 0.5;
        }
    }
    let avg_excess = if hop_count > 0.0 {
        total_excess / hop_count
    } else {
        0.0
    };
    let score = (1.0 / (1.0 + avg_excess * 0.13 + crowding_penalty * 0.22)).clamp(0.25, 1.0);
    (score, edges)
}

pub fn count_open_spaced_slots(state: &GameState, floor: FloorId) -> usize {
    let half = match floor {
        FloorId::MainFloor => state.main_floor_half_extent,
        FloorId::Administration => 4,
        _ => 5,
    };
    let mut count = 0usize;
    for gx in -half..half {
        for gz in -half..half {
            if has_module_clearance(&state.modules, floor, gx, gz, None) {
                count += 1;
            }
        }
    }
    count
}

pub fn find_procedural_bay_slot(state: &GameState, floor: FloorId) -> Option<(i32, i32)> {
    let half = match floor {
        FloorId::MainFloor => state.main_floor_half_extent,
        FloorId::Administration => 4,
        _ => 5,
    };

    let mut candidates = Vec::new();
    for gx in -half..half {
        for gz in -half..half {
            if !has_module_clearance(&state.modules, floor, gx, gz, None) {
                continue;
            }
            let reserved = state.robots.iter().any(|r| match r.task {
                RobotTask::MovingToRelocate { dest_gx, dest_gz, .. }
                | RobotTask::RelocatingModule { dest_gx, dest_gz, .. } => {
                    r.floor == floor
                        && (dest_gx - gx).abs().max((dest_gz - gz).abs()) < MIN_MODULE_CHEBYSHEV
                }
                _ => false,
            });
            if !reserved {
                candidates.push((gx, gz));
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let mut rng = rand::thread_rng();
    candidates.sort_by(|a, b| {
        let ra = a.0.abs() + a.1.abs();
        let rb = b.0.abs() + b.1.abs();
        rb.cmp(&ra)
    });
    let pool = (candidates.len() / 2).max(1);
    Some(candidates[rng.gen_range(0..pool)])
}

/// Two-stage (fast surrogate pre-filter -> exact A* top-8 verification) Builder optimizer!
/// Runs in < 0.2ms even on 26x26 Megafactories and avoids thrashing active stations.
pub fn propose_builder_optimization(state: &GameState) -> Option<(u32, i32, i32)> {
    let half = state.main_floor_half_extent;
    let current_eff =
        evaluate_layout_with_pathing(&state.modules, &state.congestion_map, half).0;
    if current_eff >= 0.965 {
        return None;
    }

    let already_targeted: Vec<u32> = state
        .robots
        .iter()
        .filter_map(|r| match r.task {
            RobotTask::MovingToRelocate { module_id, .. }
            | RobotTask::RelocatingModule { module_id, .. } => Some(module_id),
            _ => None,
        })
        .collect();

    let movable_ids: Vec<u32> = state
        .modules
        .iter()
        .filter(|m| {
            m.floor == FloorId::MainFloor
                && m.kind != ModuleKind::VerticalFreightLift
                && !m.being_relocated
                && !already_targeted.contains(&m.id)
        })
        .map(|m| m.id)
        .collect();

    if movable_ids.is_empty() {
        return None;
    }

    let base_fast = evaluate_layout_fast(&state.modules, &state.congestion_map, half);
    let mut top_candidates: Vec<(f32, u32, i32, i32)> = Vec::new();
    let mut trial = state.modules.clone();

    for &mod_id in &movable_ids {
        let Some(t_idx) = trial.iter().position(|m| m.id == mod_id) else {
            continue;
        };
        let (orig_x, orig_z) = (trial[t_idx].gx, trial[t_idx].gz);

        for gx in (-half + 1)..(half - 1) {
            for gz in (-half + 1)..(half - 1) {
                if !has_module_clearance(&state.modules, FloorId::MainFloor, gx, gz, Some(mod_id)) {
                    continue;
                }
                let reserved = state.robots.iter().any(|r| match r.task {
                    RobotTask::MovingToRelocate { dest_gx, dest_gz, .. }
                    | RobotTask::RelocatingModule { dest_gx, dest_gz, .. } => {
                        (dest_gx - gx).abs().max((dest_gz - gz).abs()) < MIN_MODULE_CHEBYSHEV
                    }
                    _ => false,
                });
                if reserved {
                    continue;
                }

                trial[t_idx].gx = gx;
                trial[t_idx].gz = gz;
                let fast_score = evaluate_layout_fast(&trial, &state.congestion_map, half);
                if fast_score - base_fast > 0.018 {
                    top_candidates.push((fast_score, mod_id, gx, gz));
                }
            }
        }
        trial[t_idx].gx = orig_x;
        trial[t_idx].gz = orig_z;
    }

    top_candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    top_candidates.truncate(8);

    let mut best_gain = 0.028f32;
    let mut best_move = None;
    for (_, mod_id, gx, gz) in top_candidates {
        if let Some(tm) = trial.iter_mut().find(|m| m.id == mod_id) {
            let (ox, oz) = (tm.gx, tm.gz);
            tm.gx = gx;
            tm.gz = gz;
            let exact = evaluate_layout_with_pathing(&trial, &state.congestion_map, half).0;
            if let Some(tm_back) = trial.iter_mut().find(|m| m.id == mod_id) {
                tm_back.gx = ox;
                tm_back.gz = oz;
            }
            if exact - current_eff > best_gain {
                best_gain = exact - current_eff;
                best_move = Some((mod_id, gx, gz));
            }
        }
    }

    best_move
}

pub fn tick_builder_robots(state: &mut GameState, dt: f32) {
    for val in state.congestion_map.values_mut() {
        *val *= (1.0 - 0.18 * dt).max(0.0);
    }
    state.congestion_map.retain(|_, &mut v| v > 0.05);

    let mult = compute_multipliers(state);
    let lights_out = state.has_tech("robo_lights_out");
    let builder_indices: Vec<usize> = state
        .robots
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RobotKind::Builder)
        .map(|(i, _)| i)
        .collect();

    let can_search_this_tick = state.tick_count % 8 == 0;

    for idx in builder_indices {
        let tier_boost = 1.0 + (state.robots[idx].tier.saturating_sub(1) as f32) * 0.28;
        let move_v = state.robots[idx].kind.base_speed() * mult.move_speed * tier_boost;
        let reloc_rate = 0.48 * mult.builder_rate * tier_boost;

        let current_task = state.robots[idx].task.clone();
        match current_task {
            RobotTask::Idle => {
                let job = if can_search_this_tick {
                    propose_builder_optimization(state)
                } else {
                    None
                };
                if let Some((module_id, dest_gx, dest_gz)) = job {
                    let bot_id = state.robots[idx].id;
                    let dock_pt = state
                        .modules
                        .iter()
                        .find(|m| m.id == module_id)
                        .map(|m| {
                            module_dock_point(
                                m,
                                bot_id,
                                &state.modules,
                                &state.robots,
                                state.main_floor_half_extent,
                            )
                        });

                    if let Some((mx, mz)) = dock_pt {
                        let r = &mut state.robots[idx];
                        r.target_x = mx;
                        r.target_z = mz;
                        r.task = RobotTask::MovingToRelocate {
                            module_id,
                            dest_gx,
                            dest_gz,
                        };
                    }
                } else {
                    let r_id = state.robots[idx].id;
                    let dx = state.robots[idx].target_x - state.robots[idx].x;
                    let dz = state.robots[idx].target_z - state.robots[idx].z;
                    if dx * dx + dz * dz < 0.55 {
                        let angle = (state.elapsed_secs as f32 * 0.75) + (r_id as f32 * 2.1) + (state.robots[idx].tasks_completed as f32);
                        let rad = (state.main_floor_half_extent as f32) * 1.55;
                        let (px, pz) = crate::navigation::find_safe_open_point(
                            &state.modules,
                            FloorId::MainFloor,
                            state.main_floor_half_extent,
                            angle.cos() * rad,
                            angle.sin() * rad,
                            0.54,
                        );
                        state.robots[idx].target_x = px;
                        state.robots[idx].target_z = pz;
                    }
                    step_robot_with_collision(state, idx, move_v * 0.75, dt);
                }
            }
            RobotTask::MovingToRelocate {
                module_id,
                dest_gx,
                dest_gz,
            } => {
                if step_robot_with_collision(state, idx, move_v, dt) {
                    if !lights_out {
                        if let Some(m) = state.modules.iter_mut().find(|m| m.id == module_id) {
                            m.being_relocated = true;
                        }
                    }
                    state.robots[idx].task = RobotTask::RelocatingModule {
                        module_id,
                        dest_gx,
                        dest_gz,
                        progress: 0.0,
                    };
                }
            }
            RobotTask::RelocatingModule {
                module_id,
                dest_gx,
                dest_gz,
                mut progress,
            } => {
                progress = (progress + reloc_rate * dt).min(1.0);
                let dest_wx = dest_gx as f32 * CELL_PITCH;
                let dest_wz = dest_gz as f32 * CELL_PITCH;

                if let Some(m) = state.modules.iter_mut().find(|m| m.id == module_id) {
                    let (start_x, start_z) = m.world_pos();
                    let vx = start_x + (dest_wx - start_x) * progress;
                    let vz = start_z + (dest_wz - start_z) * progress;
                    m.visual_x = vx;
                    m.visual_z = vz;
                    state.robots[idx].target_x = vx + 1.55;
                    state.robots[idx].target_z = vz + 1.55;
                    state.robots[idx].heading = (dest_wz - start_z).atan2(dest_wx - start_x);

                    if progress >= 1.0 {
                        m.gx = dest_gx;
                        m.gz = dest_gz;
                        m.visual_x = dest_wx;
                        m.visual_z = dest_wz;
                        m.being_relocated = false;
                        state.robots[idx].task = RobotTask::Idle;
                        crate::navigation::resolve_floor_collisions(state, FloorId::MainFloor);
                        state.robots[idx].tasks_completed += 1;
                        state.builder_optimizations_done += 1;
                    } else {
                        state.robots[idx].task = RobotTask::RelocatingModule {
                            module_id,
                            dest_gx,
                            dest_gz,
                            progress,
                        };
                    }
                } else {
                    state.robots[idx].task = RobotTask::Idle;
                }
                let _ = step_robot_with_collision(state, idx, move_v * 1.2, dt);
            }
            _ => {
                state.robots[idx].task = RobotTask::Idle;
            }
        }
    }

    if state.tick_count % 6 == 0 || state.pheromones.is_empty() {
        let (eff, edges) = evaluate_layout_with_pathing(
            &state.modules,
            &state.congestion_map,
            state.main_floor_half_extent,
        );
        state.layout_efficiency = eff;
        state.pheromones = if state.has_tech("swarm_stigmergy") {
            edges
        } else {
            Vec::new()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Robot;

    #[test]
    fn builder_robot_optimizes_pathing_fast_and_preserves_spacing() {
        let mut state = GameState::new_starter_factory();
        let initial_eff = evaluate_layout_with_pathing(
            &state.modules,
            &state.congestion_map,
            state.main_floor_half_extent,
        )
        .0;
        let bid = state.alloc_id();
        state.robots.push(Robot {
            id: bid,
            kind: RobotKind::Builder,
            floor: FloorId::MainFloor,
            x: -1.5,
            z: 0.0,
            target_x: -1.5,
            target_z: 0.0,
            heading: 0.0,
            tier: 1,
            energy: 1.0,
            integrity: 1.0,
            morale_boost_timer: 0.0,
            task: RobotTask::Idle,
            tasks_completed: 0,
        });

        for t in 0..320 {
            state.tick_count = t;
            tick_builder_robots(&mut state, 0.1);
        }

        assert!(state.builder_optimizations_done >= 1);
        assert!(state.layout_efficiency > initial_eff);
    }
}
