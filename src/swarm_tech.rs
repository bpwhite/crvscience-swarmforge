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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechBranch {
    SwarmIntelligence,
    RoboticsAndFacility,
}

#[derive(Debug, Clone)]
pub struct TechNode {
    pub id: &'static str,
    pub code: &'static str,
    pub name: &'static str,
    pub branch: TechBranch,
    pub rp_cost: f64,
    pub credit_cost: f64,
    pub prereq: Option<&'static str>,
    pub summary: &'static str,
}

pub const TECH_TREE: [TechNode; 16] = [
    // Branch A: Swarm Intelligence Upgrade Tree (S1 - S8)
    TechNode {
        id: "swarm_stigmergy",
        code: "S1",
        name: "Local Stigmergy Pheromones",
        branch: TechBranch::SwarmIntelligence,
        rp_cost: 10.0,
        credit_cost: 60.0,
        prereq: None,
        summary: "Virtual floor pheromone vectors cut idle routing delay (+18% robot speed).",
    },
    TechNode {
        id: "swarm_cnp_auction",
        code: "S2",
        name: "Contract-Net Task Auction",
        branch: TechBranch::SwarmIntelligence,
        rp_cost: 28.0,
        credit_cost: 140.0,
        prereq: Some("swarm_stigmergy"),
        summary: "Decentralized task bidding eliminates duplicate hauler trips (+22% handoff efficiency).",
    },
    TechNode {
        id: "swarm_peloton",
        code: "S3",
        name: "Slipstream Peloton Kinematics",
        branch: TechBranch::SwarmIntelligence,
        rp_cost: 55.0,
        credit_cost: 260.0,
        prereq: Some("swarm_cnp_auction"),
        summary: "Collision-free synchronized AGV convoys (+25% transit velocity).",
    },
    TechNode {
        id: "swarm_multi_arm",
        code: "S4",
        name: "Cooperative Multi-Arm Cells",
        branch: TechBranch::SwarmIntelligence,
        rp_cost: 95.0,
        credit_cost: 440.0,
        prereq: Some("swarm_peloton"),
        summary: "Multiple Assembler Bots co-operate on shared stations (+30% assembly synergy).",
    },
    TechNode {
        id: "swarm_aco_layout",
        code: "S5",
        name: "Ant-Colony Layout Annealing",
        branch: TechBranch::SwarmIntelligence,
        rp_cost: 150.0,
        credit_cost: 700.0,
        prereq: Some("swarm_multi_arm"),
        summary: "Builder Bots use ACO + simulated annealing to relocate equipment 2x faster.",
    },
    TechNode {
        id: "swarm_vertical_sync",
        code: "S6",
        name: "Cross-Floor Lift Synchronization",
        branch: TechBranch::SwarmIntelligence,
        rp_cost: 230.0,
        credit_cost: 1100.0,
        prereq: Some("swarm_aco_layout"),
        summary: "Predictive vertical shaft staging across B1, F1, F2, and F3 (+28% throughput).",
    },
    TechNode {
        id: "swarm_bft_mesh",
        code: "S7",
        name: "Emergent BFT Mesh Consensus",
        branch: TechBranch::SwarmIntelligence,
        rp_cost: 360.0,
        credit_cost: 1750.0,
        prereq: Some("swarm_vertical_sync"),
        summary: "Peer-to-peer holographic state mesh links nearby bots (+35% global efficiency).",
    },
    TechNode {
        id: "swarm_singularity",
        code: "S8",
        name: "Harmonic Swarm Singularity",
        branch: TechBranch::SwarmIntelligence,
        rp_cost: 580.0,
        credit_cost: 2900.0,
        prereq: Some("swarm_bft_mesh"),
        summary: "Phase-locked factory hive-mind (+50% global output & unlocks Tier-5 Singularity Core).",
    },
    // Branch B: Robotics, Multi-Floor & Assembly Tech Tree (R1 - R8)
    TechNode {
        id: "robo_servo_kinematics",
        code: "R1",
        name: "Precision Servo Kinematics",
        branch: TechBranch::RoboticsAndFacility,
        rp_cost: 12.0,
        credit_cost: 75.0,
        prereq: None,
        summary: "Upgraded harmonic drives accelerate station cycle rates (+25% work speed).",
    },
    TechNode {
        id: "robo_modular_line",
        code: "R2",
        name: "Multi-Stage Assembly Line",
        branch: TechBranch::RoboticsAndFacility,
        rp_cost: 32.0,
        credit_cost: 180.0,
        prereq: Some("robo_servo_kinematics"),
        summary: "Unlocks Tier-2 Autonomous Drone Core & specialized Sub/Final Assembly modules.",
    },
    TechNode {
        id: "robo_basement_hv",
        code: "R3",
        name: "HV Busbars & Cryo-Plumbing",
        branch: TechBranch::RoboticsAndFacility,
        rp_cost: 60.0,
        credit_cost: 320.0,
        prereq: Some("robo_modular_line"),
        summary: "Boosts Basement substation capacity (+35% kW) and coolant manifold pressure.",
    },
    TechNode {
        id: "robo_floor2_labs",
        code: "R4",
        name: "Accelerated Research Complex",
        branch: TechBranch::RoboticsAndFacility,
        rp_cost: 90.0,
        credit_cost: 480.0,
        prereq: Some("robo_basement_hv"),
        summary: "Doubles Floor 2 Lab Bot RP synthesis and unlocks Tier-3 Quantum LiDAR Array.",
    },
    TechNode {
        id: "robo_floor3_admin",
        code: "R5",
        name: "Algorithmic C-Suite (Floor 3)",
        branch: TechBranch::RoboticsAndFacility,
        rp_cost: 140.0,
        credit_cost: 760.0,
        prereq: Some("robo_floor2_labs"),
        summary: "Boosts Floor 3 Executive Bot sale multipliers (+30%) and auto-replenishes contracts.",
    },
    TechNode {
        id: "robo_bay_expansion",
        code: "R6",
        name: "Wide-Span Structural Bays",
        branch: TechBranch::RoboticsAndFacility,
        rp_cost: 210.0,
        credit_cost: 1200.0,
        prereq: Some("robo_floor3_admin"),
        summary: "Unlocks Tier-4 Biomimetic Exo-Frame and reduces Floor 1 bay expansion cost by 30%.",
    },
    TechNode {
        id: "robo_holo_litho",
        code: "R7",
        name: "Femtosecond Laser Metrology",
        branch: TechBranch::RoboticsAndFacility,
        rp_cost: 340.0,
        credit_cost: 1950.0,
        prereq: Some("robo_bay_expansion"),
        summary: "Zero-defect QA packaging (+35% product value) and faster Maintenance Bot repairs.",
    },
    TechNode {
        id: "robo_lights_out",
        code: "R8",
        name: "Lights-Out Megafactory",
        branch: TechBranch::RoboticsAndFacility,
        rp_cost: 520.0,
        credit_cost: 2800.0,
        prereq: Some("robo_holo_litho"),
        summary: "Zero-downtime Builder hot-swaps; machines keep producing even while being relocated.",
    },
];

#[derive(Debug, Clone, Copy)]
pub struct SwarmMultipliers {
    pub move_speed: f32,
    pub work_speed: f32,
    pub builder_rate: f32,
    pub research_rate: f32,
    pub sale_value: f32,
    pub power_capacity_mult: f32,
    pub coherence_index: f32,
    pub max_product_tier: u8,
}

pub fn compute_multipliers(state: &GameState) -> SwarmMultipliers {
    let mut move_speed = 1.0f32;
    let mut work_speed = 1.0f32;
    let mut builder_rate = 1.0f32;
    let mut research_rate = 1.0f32;
    let mut sale_value = 1.0f32;
    let mut power_capacity_mult = 1.0f32;
    let mut coherence = 1.0f32;
    let mut max_tier = 1u8;

    if state.has_tech("swarm_stigmergy") {
        move_speed *= 1.18;
        coherence += 0.25;
    }
    if state.has_tech("swarm_cnp_auction") {
        work_speed *= 1.22;
        coherence += 0.35;
    }
    if state.has_tech("swarm_peloton") {
        move_speed *= 1.25;
        coherence += 0.45;
    }
    if state.has_tech("swarm_multi_arm") {
        work_speed *= 1.30;
        coherence += 0.55;
    }
    if state.has_tech("swarm_aco_layout") {
        builder_rate *= 2.0;
        coherence += 0.70;
    }
    if state.has_tech("swarm_vertical_sync") {
        work_speed *= 1.28;
        move_speed *= 1.12;
        coherence += 0.85;
    }
    if state.has_tech("swarm_bft_mesh") {
        work_speed *= 1.35;
        research_rate *= 1.25;
        coherence += 1.10;
    }
    if state.has_tech("swarm_singularity") {
        work_speed *= 1.50;
        move_speed *= 1.30;
        sale_value *= 1.25;
        coherence += 1.75;
        max_tier = max_tier.max(5);
    }

    if state.has_tech("robo_servo_kinematics") {
        work_speed *= 1.25;
    }
    if state.has_tech("robo_modular_line") {
        max_tier = max_tier.max(2);
    }
    if state.has_tech("robo_basement_hv") {
        power_capacity_mult *= 1.35;
    }
    if state.has_tech("robo_floor2_labs") {
        research_rate *= 2.0;
        max_tier = max_tier.max(3);
    }
    if state.has_tech("robo_floor3_admin") {
        sale_value *= 1.30;
    }
    if state.has_tech("robo_bay_expansion") {
        max_tier = max_tier.max(4);
    }
    if state.has_tech("robo_holo_litho") {
        sale_value *= 1.35;
    }

    SwarmMultipliers {
        move_speed,
        work_speed,
        builder_rate,
        research_rate,
        sale_value,
        power_capacity_mult,
        coherence_index: coherence,
        max_product_tier: max_tier,
    }
}

pub fn can_unlock_tech(state: &GameState, node_id: &str) -> bool {
    if state.has_tech(node_id) {
        return false;
    }
    let Some(node) = TECH_TREE.iter().find(|n| n.id == node_id) else {
        return false;
    };
    if let Some(pre) = node.prereq {
        if !state.has_tech(pre) {
            return false;
        }
    }
    state.research_points >= node.rp_cost && state.credits >= node.credit_cost
}

pub fn unlock_tech(state: &mut GameState, node_id: &str) -> Result<(), String> {
    let Some(node) = TECH_TREE.iter().find(|n| n.id == node_id) else {
        return Err("Unknown tech node.".to_string());
    };
    if state.has_tech(node_id) {
        return Err("Tech already researched.".to_string());
    }
    if let Some(pre) = node.prereq {
        if !state.has_tech(pre) {
            return Err(format!("Requires prerequisite tech '{pre}'."));
        }
    }
    if state.research_points < node.rp_cost || state.credits < node.credit_cost {
        return Err("Insufficient Research Points or Credits.".to_string());
    }

    state.research_points -= node.rp_cost;
    state.credits -= node.credit_cost;
    state.unlocked_techs.push(node.id.to_string());

    let mult = compute_multipliers(state);
    state.swarm_coherence = mult.coherence_index;
    if mult.max_product_tier > state.active_product_tier {
        state.active_product_tier = mult.max_product_tier;
    }
    state.push_log(format!("Researched [{}] {}!", node.code, node.name));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tech_unlock_progression_and_swarm_scaling() {
        let mut state = GameState::new_starter_factory();
        state.credits = 5_000.0;
        state.research_points = 500.0;

        assert!(can_unlock_tech(&state, "swarm_stigmergy"));
        assert!(!can_unlock_tech(&state, "swarm_cnp_auction"));

        unlock_tech(&mut state, "swarm_stigmergy").expect("S1 unlock");
        assert!(can_unlock_tech(&state, "swarm_cnp_auction"));
        unlock_tech(&mut state, "swarm_cnp_auction").expect("S2 unlock");

        let m = compute_multipliers(&state);
        assert!(m.move_speed > 1.15);
        assert!(m.work_speed > 1.20);
        assert!(m.coherence_index > 1.5);
    }
}
