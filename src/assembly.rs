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

use crate::builder_ai::tick_builder_robots;
use crate::floors::tick_support_floors;
use crate::model::{FloorId, GameState, ModuleKind, RobotKind, RobotTask, PRODUCT_CATALOG};
use crate::navigation::{module_dock_point, step_robot_with_collision};
use crate::swarm_tech::compute_multipliers;

pub fn tick_factory(state: &mut GameState, dt: f32) {
    let dt = dt.clamp(0.001, 0.25);
    state.tick_count += 1;
    state.elapsed_secs += dt as f64;

    tick_support_floors(state, dt);
    crate::disasters::tick_disasters_repair_and_drones(state, dt);
    tick_builder_robots(state, dt);
    tick_assembly_line(state, dt);
    check_achievements(state);
}

fn tick_assembly_line(state: &mut GameState, dt: f32) {
    let mult = compute_multipliers(state);
    state.swarm_coherence = mult.coherence_index;

    let grid_factor = if state.power_demand_kw <= state.power_supply_kw {
        1.0
    } else {
        (state.power_supply_kw / state.power_demand_kw).clamp(0.35, 1.0)
    };
    let pressure_bonus = (state.coolant_pressure_psi / 90.0).clamp(0.85, 1.35);
    let layout_bonus = 0.65 + 0.75 * state.layout_efficiency;

    let bp = &PRODUCT_CATALOG[(state.active_product_tier.saturating_sub(1) as usize)
        .min(PRODUCT_CATALOG.len() - 1)];

    let mut active_stages: Vec<u8> = state
        .modules
        .iter()
        .filter(|m| m.floor == FloorId::MainFloor && !m.being_relocated)
        .filter_map(|m| m.kind.pipeline_stage())
        .collect();
    active_stages.sort_unstable();
    active_stages.dedup();

    let first_prod_stage = active_stages
        .iter()
        .copied()
        .find(|&s| s <= 4)
        .unwrap_or(1);

    let sync_boost = if state.has_tech("swarm_vertical_sync") {
        2
    } else {
        1
    };
    for m in state.modules.iter_mut() {
        if m.floor == FloorId::MainFloor
            && m.kind.pipeline_stage() == Some(first_prod_stage)
            && m.input_buffer < 6
            && state.raw_material_stock >= 1.0
        {
            let pull = sync_boost.min((6 - m.input_buffer) as usize) as u32;
            let actual = pull.min(state.raw_material_stock.floor() as u32);
            if actual > 0 {
                state.raw_material_stock -= actual as f32;
                m.input_buffer += actual;
            }
        }
    }

    let multi_arm = state.has_tech("swarm_multi_arm");
    let auto_baseline = if state.has_tech("robo_modular_line") {
        0.28
    } else {
        0.08
    };

    for m_idx in 0..state.modules.len() {
        let m = &state.modules[m_idx];
        if m.floor != FloorId::MainFloor || m.being_relocated {
            continue;
        }
        let Some(stage) = m.kind.pipeline_stage() else {
            continue;
        };
        if stage == 5 || m.input_buffer == 0 || m.output_buffer >= 8 {
            continue;
        }

        let mod_id = m.id;
        let mod_pos = (m.visual_x, m.visual_z);
        // Robots park at exterior docking aprons (~1.46 units from module center)
        let operators = state
            .robots
            .iter()
            .filter(|r| {
                matches!(r.task, RobotTask::Operating { module_id, .. } if module_id == mod_id)
                    && ((r.x - mod_pos.0).powi(2) + (r.z - mod_pos.1).powi(2)) < 5.2
            })
            .count();

        if operators == 0 && auto_baseline <= 0.0 {
            continue;
        }

        let base_auto = if state.modules[m_idx].kind == ModuleKind::RawIntakeHopper { 1.35 } else { auto_baseline };
        let coop_factor = if operators == 0 {
            base_auto
        } else if multi_arm && operators >= 2 {
            (operators as f32) * 1.30
        } else {
            1.0 + (operators.saturating_sub(1) as f32) * 0.65
        };

        let condition = state.modules[m_idx].condition;
        let has_hazard = state.disasters.iter().any(|d| d.module_id == mod_id);
        let hazard_mult = if has_hazard { 0.20 } else { 1.0 };
        let rate = (0.55 / bp.cycle_difficulty) * hazard_mult
            * coop_factor
            * mult.work_speed
            * grid_factor
            * pressure_bonus
            * layout_bonus
            * condition;

        let m_mut = &mut state.modules[m_idx];
        m_mut.work_progress += rate * dt;
        if m_mut.work_progress >= 1.0 {
            m_mut.work_progress -= 1.0;
            m_mut.input_buffer = m_mut.input_buffer.saturating_sub(1);
            m_mut.output_buffer += 1;
        }
    }

    let mut delivered_this_tick = 0u32;

    for r_idx in 0..state.robots.len() {
        let kind = state.robots[r_idx].kind;
        if !matches!(
            kind,
            RobotKind::BasicWorker | RobotKind::Assembler | RobotKind::Hauler
        ) {
            continue;
        }

        let tier_mult = 1.0 + (state.robots[r_idx].tier.saturating_sub(1) as f32) * 0.25;
        let health_mult = 0.45 + 0.55 * state.robots[r_idx].integrity;
        let cheer_mult = if state.robots[r_idx].morale_boost_timer > 0.0 { 1.22 } else { 1.0 };
        let move_speed = kind.base_speed() * mult.move_speed * tier_mult * grid_factor * health_mult * cheer_mult;
        let task = state.robots[r_idx].task.clone();
        let bot_id = state.robots[r_idx].id;

        match task {
            RobotTask::Idle => {
                let can_haul = matches!(kind, RobotKind::Hauler | RobotKind::BasicWorker)
                    || !state
                        .robots
                        .iter()
                        .any(|o| matches!(o.kind, RobotKind::Hauler | RobotKind::BasicWorker));

                let mut assigned = false;
                if can_haul {
                    if let Some((src_id, src_stage, dst_id)) =
                        find_haul_job(&state.modules, &active_stages)
                    {
                        if let Some(src_m) = state.modules.iter_mut().find(|m| m.id == src_id) {
                            src_m.output_buffer = src_m.output_buffer.saturating_sub(1);
                        }
                        if let Some((dx, dz)) = state
                            .modules
                            .iter()
                            .find(|m| m.id == dst_id)
                            .map(|m| module_dock_point(m, bot_id, &state.modules, &state.robots, state.main_floor_half_extent))
                        {
                            let r = &mut state.robots[r_idx];
                            r.target_x = dx;
                            r.target_z = dz;
                            r.task = RobotTask::CarryingPart {
                                stage: src_stage,
                                product_tier: state.active_product_tier,
                                target_module_id: dst_id,
                            };
                            assigned = true;
                        }
                    }
                }

                if !assigned && matches!(kind, RobotKind::Assembler | RobotKind::BasicWorker) {
                    if let Some((mid, (dx, dz))) = state
                        .modules
                        .iter()
                        .filter(|m| {
                            m.floor == FloorId::MainFloor
                                && !m.being_relocated
                                && m.kind.pipeline_stage().map(|s| s <= 4).unwrap_or(false)
                                && m.input_buffer > 0
                                && m.output_buffer < 8
                        })
                        .max_by_key(|m| (m.kind.pipeline_stage().unwrap_or(0), m.input_buffer))
                        .map(|m| (m.id, module_dock_point(m, bot_id, &state.modules, &state.robots, state.main_floor_half_extent)))
                    {
                        let r = &mut state.robots[r_idx];
                        r.target_x = dx;
                        r.target_z = dz;
                        r.task = RobotTask::Operating {
                            module_id: mid,
                            timer: 2.2,
                        };
                        assigned = true;
                    }
                }

                if !assigned {
                    let _ = step_robot_with_collision(state, r_idx, move_speed * 0.6, dt);
                }
            }
            RobotTask::Operating {
                module_id,
                mut timer,
            } => {
                let arrived = step_robot_with_collision(state, r_idx, move_speed, dt);
                if arrived {
                    if let Some(m) = state.modules.iter().find(|m| m.id == module_id) {
                        // Face the station while operating!
                        state.robots[r_idx].heading =
                            (m.visual_z - state.robots[r_idx].z).atan2(m.visual_x - state.robots[r_idx].x);
                    }
                    timer -= dt;
                    let still_has_work = state
                        .modules
                        .iter()
                        .find(|m| m.id == module_id && !m.being_relocated)
                        .map(|m| m.input_buffer > 0 && m.output_buffer < 8)
                        .unwrap_or(false);

                    let solo_haul_needed = kind == RobotKind::BasicWorker
                        && !state.robots.iter().any(|o| o.kind == RobotKind::Hauler)
                        && state
                            .modules
                            .iter()
                            .any(|m| m.id == module_id && m.output_buffer > 0);

                    if timer <= 0.0 || !still_has_work || solo_haul_needed {
                        state.robots[r_idx].task = RobotTask::Idle;
                        state.robots[r_idx].tasks_completed += 1;
                    } else {
                        state.robots[r_idx].task = RobotTask::Operating { module_id, timer };
                    }
                }
            }
            RobotTask::CarryingPart {
                stage,
                product_tier,
                target_module_id,
            } => {
                // Only refresh dock point if the target module itself moved (e.g. Builder relocation)
                if let Some(m) = state.modules.iter().find(|m| m.id == target_module_id) {
                    let cur_d = (state.robots[r_idx].target_x - m.visual_x).hypot(state.robots[r_idx].target_z - m.visual_z);
                    if !(1.35..=2.05).contains(&cur_d) {
                        let (dx, dz) = module_dock_point(m, bot_id, &state.modules, &state.robots, state.main_floor_half_extent);
                        state.robots[r_idx].target_x = dx;
                        state.robots[r_idx].target_z = dz;
                    }
                }
                let arrived = step_robot_with_collision(state, r_idx, move_speed, dt);
                if arrived {
                    if let Some(dst) = state.modules.iter_mut().find(|m| m.id == target_module_id) {
                        if dst.kind == ModuleKind::DeliveryDock {
                            delivered_this_tick += 1;
                        } else {
                            dst.input_buffer += 1;
                        }
                    }
                    state.robots[r_idx].task = RobotTask::Idle;
                    state.robots[r_idx].tasks_completed += 1;
                    let _ = (stage, product_tier);
                }
            }
            RobotTask::SeekingMaintenance { .. } => {} // Handled in disasters::tick_disasters_repair_and_drones
            _ => {
                state.robots[r_idx].task = RobotTask::Idle;
            }
        }
    }

    if delivered_this_tick > 0 {
        complete_deliveries(state, delivered_this_tick);
    }
}

fn find_haul_job(
    modules: &[crate::model::FactoryModule],
    active_stages: &[u8],
) -> Option<(u32, u8, u32)> {
    let mut ordered: Vec<&crate::model::FactoryModule> = modules.iter().collect();
    ordered.sort_by_key(|m| std::cmp::Reverse(m.kind.pipeline_stage().unwrap_or(0)));
    for m in ordered {
        if m.floor != FloorId::MainFloor || m.being_relocated || m.output_buffer == 0 {
            continue;
        }
        let Some(st) = m.kind.pipeline_stage() else {
            continue;
        };
        let Some(&next_stage) = active_stages.iter().find(|&&s| s > st) else {
            continue;
        };
        let dst = modules
            .iter()
            .filter(|d| {
                d.floor == FloorId::MainFloor
                    && !d.being_relocated
                    && d.kind.pipeline_stage() == Some(next_stage)
                    && (d.kind == ModuleKind::DeliveryDock || d.input_buffer < 8)
            })
            .min_by_key(|d| (d.gx - m.gx).abs() + (d.gz - m.gz).abs());

        if let Some(target) = dst {
            return Some((m.id, st, target.id));
        }
    }
    None
}

pub fn complete_deliveries(state: &mut GameState, units: u32) {
    let bp = &PRODUCT_CATALOG[(state.active_product_tier.saturating_sub(1) as usize)
        .min(PRODUCT_CATALOG.len() - 1)];

    let mut distinct_stages: Vec<u8> = state
        .modules
        .iter()
        .filter_map(|m| m.kind.pipeline_stage())
        .filter(|&s| s <= 4)
        .collect();
    distinct_stages.sort_unstable();
    distinct_stages.dedup();
    let integration_bonus = 1.0 + (distinct_stages.len().saturating_sub(1) as f64) * 0.22;

    let unit_price = bp.base_value
        * integration_bonus
        * (state.admin_margin_multiplier as f64)
        * (0.85 + 0.30 * state.layout_efficiency as f64);

    let earned = unit_price * (units as f64);
    state.credits += earned;
    state.total_revenue += earned;
    state.total_products_shipped += units as u64;
    // Launch outbound DeliveryDock cargo drone sortie!
    for m in state.modules.iter_mut() {
        if m.kind == ModuleKind::DeliveryDock && m.drone_sortie_progress == 0.0 {
            m.drone_sortie_progress = 0.01;
            break;
        }
    }
    state.research_points += 0.45 * (units as f64);

    let mut newly_completed = Vec::new();
    for c in state.contracts.iter_mut() {
        if !c.completed && c.product_tier <= state.active_product_tier {
            c.delivered_units = (c.delivered_units + units).min(c.required_units);
            if c.delivered_units >= c.required_units {
                c.completed = true;
                state.credits += c.bonus_credits;
                state.total_revenue += c.bonus_credits;
                state.research_points += c.bonus_rp;
                newly_completed.push((c.title.clone(), c.bonus_credits, c.bonus_rp));
            }
        }
    }

    for (title, bc, brp) in newly_completed {
        state.push_log(format!(
            "Contract Complete: {title} (+₡{bc:.0}, +{brp:.0} RP)!"
        ));
    }
}

fn check_achievements(state: &mut GameState) {
    let checks = [
        ("first_sale", "First Delivery", state.total_products_shipped >= 1),
        ("swarm_ten", "10-Bot Swarm", state.robots.len() >= 10),
        (
            "builder_architect",
            "Autonomous Layout Architect",
            state.builder_optimizations_done >= 3,
        ),
        (
            "vertical_four_floors",
            "Four-Floor Vertical Integration",
            state.robots.iter().any(|r| r.kind == RobotKind::Maintenance)
                && state.robots.iter().any(|r| r.kind == RobotKind::Researcher)
                && state.robots.iter().any(|r| r.kind == RobotKind::Administrator),
        ),
        (
            "expanded_megabay",
            "Expanded Floor 1 Bay",
            state.main_floor_half_extent >= 7,
        ),
    ];

    for (id, label, cond) in checks {
        if cond && !state.achievements.iter().any(|a| a == id) {
            state.achievements.push(id.to_string());
            state.push_log(format!("Milestone Unlocked: {label}!"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_starter_robot_produces_and_sells_with_obstacle_pathing() {
        let mut state = GameState::new_starter_factory();
        let start_credits = state.credits;

        for _ in 0..500 {
            tick_factory(&mut state, 0.1);
        }

        assert!(state.total_products_shipped >= 1);
        assert!(state.credits > start_credits);
    }
}
