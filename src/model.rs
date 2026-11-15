#![forbid(unsafe_code)]

use crate::navigation::CELL_PITCH;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_one() -> f32 {
    1.0
}
fn default_tier_one() -> u8 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FloorId {
    Basement,
    MainFloor,
    TechResearch,
    Administration,
}

impl FloorId {
    pub const ALL: [FloorId; 4] = [
        FloorId::Basement,
        FloorId::MainFloor,
        FloorId::TechResearch,
        FloorId::Administration,
    ];

    pub fn index(self) -> usize {
        match self {
            FloorId::Basement => 0,
            FloorId::MainFloor => 1,
            FloorId::TechResearch => 2,
            FloorId::Administration => 3,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            FloorId::Basement => "B1",
            FloorId::MainFloor => "F1",
            FloorId::TechResearch => "F2",
            FloorId::Administration => "F3",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            FloorId::Basement => "B1: Utility Grid, Piping & Storage",
            FloorId::MainFloor => "F1: Main Robotic Assembly Floor",
            FloorId::TechResearch => "F2: Tech & Swarm Research Labs",
            FloorId::Administration => "F3: Robotic Administration",
        }
    }

    pub fn world_y(self) -> f32 {
        match self {
            FloorId::Basement => -6.2,
            FloorId::MainFloor => 0.0,
            FloorId::TechResearch => 6.2,
            FloorId::Administration => 12.4,
        }
    }
}

/// Industrial disaster & hazard categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DisasterKind {
    ThermalFire,
    GridOverload,
    CoolantLeak,
    LogicGlitch,
}

impl DisasterKind {
    pub fn label(self) -> &'static str {
        match self {
            DisasterKind::ThermalFire => "Thermal Station Fire",
            DisasterKind::GridOverload => "HV Arc-Flash Overload",
            DisasterKind::CoolantLeak => "Cryo Coolant Rupture",
            DisasterKind::LogicGlitch => "Firmware Logic Glitch",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDisaster {
    pub id: u32,
    pub kind: DisasterKind,
    pub floor: FloorId,
    pub module_id: u32,
    pub x: f32,
    pub z: f32,
    pub severity: f32,
    #[serde(default)]
    pub suppressed_by_tower: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RobotKind {
    BasicWorker,
    Assembler,
    Hauler,
    Builder,
    RepairBot,
    MotivationDrone,
    Maintenance,
    Researcher,
    Administrator,
}

impl RobotKind {
    pub fn label(self) -> &'static str {
        match self {
            RobotKind::BasicWorker => "Mk-I Basic Bot",
            RobotKind::Assembler => "Assembler Bot",
            RobotKind::Hauler => "AGV Hauler Bot",
            RobotKind::Builder => "Builder Architect Bot",
            RobotKind::RepairBot => "Field Repair & Fire Bot",
            RobotKind::MotivationDrone => "CheerBot Motivation Drone",
            RobotKind::Maintenance => "Grid Maintenance Bot",
            RobotKind::Researcher => "Lab Research Bot",
            RobotKind::Administrator => "Admin Executive Bot",
        }
    }

    pub fn home_floor(self) -> FloorId {
        match self {
            RobotKind::BasicWorker
            | RobotKind::Assembler
            | RobotKind::Hauler
            | RobotKind::Builder
            | RobotKind::RepairBot
            | RobotKind::MotivationDrone => FloorId::MainFloor,
            RobotKind::Maintenance => FloorId::Basement,
            RobotKind::Researcher => FloorId::TechResearch,
            RobotKind::Administrator => FloorId::Administration,
        }
    }

    pub fn base_cost(self) -> f64 {
        match self {
            RobotKind::BasicWorker => 35.0,
            RobotKind::Assembler => 85.0,
            RobotKind::Hauler => 70.0,
            RobotKind::Builder => 130.0,
            RobotKind::RepairBot => 90.0,
            RobotKind::MotivationDrone => 115.0,
            RobotKind::Maintenance => 95.0,
            RobotKind::Researcher => 150.0,
            RobotKind::Administrator => 210.0,
        }
    }

    pub fn base_speed(self) -> f32 {
        match self {
            RobotKind::BasicWorker => 2.05,
            RobotKind::Assembler => 2.55,
            RobotKind::Hauler => 3.55,
            RobotKind::Builder => 2.80,
            RobotKind::RepairBot => 3.15,
            RobotKind::MotivationDrone => 3.40,
            RobotKind::Maintenance => 2.40,
            RobotKind::Researcher => 2.25,
            RobotKind::Administrator => 2.15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RobotTask {
    Idle,
    Operating { module_id: u32, timer: f32 },
    CarryingPart {
        stage: u8,
        product_tier: u8,
        target_module_id: u32,
    },
    MovingToRelocate {
        module_id: u32,
        dest_gx: i32,
        dest_gz: i32,
    },
    RelocatingModule {
        module_id: u32,
        dest_gx: i32,
        dest_gz: i32,
        progress: f32,
    },
    SeekingMaintenance {
        station_id: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Robot {
    pub id: u32,
    pub kind: RobotKind,
    pub floor: FloorId,
    pub x: f32,
    pub z: f32,
    pub target_x: f32,
    pub target_z: f32,
    pub heading: f32,
    pub tier: u8,
    pub energy: f32,
    #[serde(default = "default_one")]
    pub integrity: f32,
    #[serde(default)]
    pub morale_boost_timer: f32,
    pub task: RobotTask,
    pub tasks_completed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleKind {
    VerticalFreightLift,
    ProtoBench,
    RawIntakeHopper,
    FabricationCell,
    SubAssemblyCell,
    FinalAssemblyCell,
    QualityPackagingBay,
    DeliveryDock,
    BotRepairStation,
    FireSuppressionTower,
    StorageSilo,
    PowerSubstation,
    PlumbingChiller,
    HydraulicPipingManifold,
    MaintenanceBay,
    SwarmSimMainframe,
    MaterialsLabRig,
    HoloBlueprintBench,
    HoloBoardroomTable,
    ContractDispatchTerminal,
    SupplyHedgeServer,
}

impl ModuleKind {
    pub fn label(self) -> &'static str {
        match self {
            ModuleKind::VerticalFreightLift => "Vertical Freight & Utility Shaft",
            ModuleKind::ProtoBench => "Universal Proto-Bench",
            ModuleKind::RawIntakeHopper => "Raw Material Intake",
            ModuleKind::FabricationCell => "CNC Fabrication Cell",
            ModuleKind::SubAssemblyCell => "SMT Sub-Assembly Cell",
            ModuleKind::FinalAssemblyCell => "Robotic Final Assembly",
            ModuleKind::QualityPackagingBay => "Laser QA & Packaging",
            ModuleKind::DeliveryDock => "Autonomous Delivery Dock",
            ModuleKind::BotRepairStation => "Autonomous Bot Repair Bay",
            ModuleKind::FireSuppressionTower => "Cryo-Foam Suppression Turret",
            ModuleKind::StorageSilo => "Automated Storage Silo",
            ModuleKind::PowerSubstation => "HV Power Grid Substation",
            ModuleKind::PlumbingChiller => "Closed-Loop Plumbing & Chiller",
            ModuleKind::HydraulicPipingManifold => "Hydraulic & Cryo Piping Manifold",
            ModuleKind::MaintenanceBay => "Diagnostic Maintenance Bay",
            ModuleKind::SwarmSimMainframe => "Neural Swarm Mainframe",
            ModuleKind::MaterialsLabRig => "Materials Stress-Test Rig",
            ModuleKind::HoloBlueprintBench => "Holo-Lithography Lab",
            ModuleKind::HoloBoardroomTable => "Robotic Executive Boardroom",
            ModuleKind::ContractDispatchTerminal => "Algorithmic Contract Terminal",
            ModuleKind::SupplyHedgeServer => "Autonomous Procurement Server",
        }
    }

    pub fn floor(self) -> FloorId {
        match self {
            ModuleKind::VerticalFreightLift
            | ModuleKind::ProtoBench
            | ModuleKind::RawIntakeHopper
            | ModuleKind::FabricationCell
            | ModuleKind::SubAssemblyCell
            | ModuleKind::FinalAssemblyCell
            | ModuleKind::QualityPackagingBay
            | ModuleKind::DeliveryDock
            | ModuleKind::BotRepairStation
            | ModuleKind::FireSuppressionTower => FloorId::MainFloor,
            ModuleKind::StorageSilo
            | ModuleKind::PowerSubstation
            | ModuleKind::PlumbingChiller
            | ModuleKind::HydraulicPipingManifold
            | ModuleKind::MaintenanceBay => FloorId::Basement,
            ModuleKind::SwarmSimMainframe
            | ModuleKind::MaterialsLabRig
            | ModuleKind::HoloBlueprintBench => FloorId::TechResearch,
            ModuleKind::HoloBoardroomTable
            | ModuleKind::ContractDispatchTerminal
            | ModuleKind::SupplyHedgeServer => FloorId::Administration,
        }
    }

    pub fn pipeline_stage(self) -> Option<u8> {
        match self {
            ModuleKind::RawIntakeHopper => Some(0),
            ModuleKind::ProtoBench | ModuleKind::FabricationCell => Some(1),
            ModuleKind::SubAssemblyCell => Some(2),
            ModuleKind::FinalAssemblyCell => Some(3),
            ModuleKind::QualityPackagingBay => Some(4),
            ModuleKind::DeliveryDock => Some(5),
            _ => None,
        }
    }

    pub fn base_cost(self) -> f64 {
        match self {
            ModuleKind::VerticalFreightLift => 0.0,
            ModuleKind::ProtoBench => 45.0,
            ModuleKind::RawIntakeHopper => 60.0,
            ModuleKind::FabricationCell => 95.0,
            ModuleKind::SubAssemblyCell => 135.0,
            ModuleKind::FinalAssemblyCell => 175.0,
            ModuleKind::QualityPackagingBay => 210.0,
            ModuleKind::DeliveryDock => 160.0,
            ModuleKind::BotRepairStation => 75.0,
            ModuleKind::FireSuppressionTower => 85.0,
            ModuleKind::StorageSilo => 80.0,
            ModuleKind::PowerSubstation => 120.0,
            ModuleKind::PlumbingChiller => 110.0,
            ModuleKind::HydraulicPipingManifold => 145.0,
            ModuleKind::MaintenanceBay => 105.0,
            ModuleKind::SwarmSimMainframe => 190.0,
            ModuleKind::MaterialsLabRig => 165.0,
            ModuleKind::HoloBlueprintBench => 240.0,
            ModuleKind::HoloBoardroomTable => 260.0,
            ModuleKind::ContractDispatchTerminal => 220.0,
            ModuleKind::SupplyHedgeServer => 250.0,
        }
    }

    pub fn power_draw_kw(self) -> f32 {
        match self {
            ModuleKind::PowerSubstation => -45.0,
            ModuleKind::VerticalFreightLift => 4.0,
            ModuleKind::ProtoBench => 3.5,
            ModuleKind::RawIntakeHopper => 2.5,
            ModuleKind::FabricationCell => 9.0,
            ModuleKind::SubAssemblyCell => 7.5,
            ModuleKind::FinalAssemblyCell => 11.0,
            ModuleKind::QualityPackagingBay => 6.0,
            ModuleKind::DeliveryDock => 3.0,
            ModuleKind::BotRepairStation => 3.5,
            ModuleKind::FireSuppressionTower => 2.0,
            ModuleKind::StorageSilo => 2.0,
            ModuleKind::PlumbingChiller => 5.5,
            ModuleKind::HydraulicPipingManifold => 4.5,
            ModuleKind::MaintenanceBay => 3.0,
            ModuleKind::SwarmSimMainframe => 12.0,
            ModuleKind::MaterialsLabRig => 8.0,
            ModuleKind::HoloBlueprintBench => 10.0,
            ModuleKind::HoloBoardroomTable => 5.0,
            ModuleKind::ContractDispatchTerminal => 4.5,
            ModuleKind::SupplyHedgeServer => 6.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryModule {
    pub id: u32,
    pub kind: ModuleKind,
    pub floor: FloorId,
    pub gx: i32,
    pub gz: i32,
    pub visual_x: f32,
    pub visual_z: f32,
    pub input_buffer: u32,
    pub output_buffer: u32,
    pub work_progress: f32,
    pub condition: f32,
    pub being_relocated: bool,
    pub level: u8,
    #[serde(default)]
    pub drone_sortie_progress: f32,
}

impl FactoryModule {
    pub fn new(id: u32, kind: ModuleKind, floor: FloorId, gx: i32, gz: i32) -> Self {
        Self {
            id,
            kind,
            floor,
            gx,
            gz,
            visual_x: gx as f32 * CELL_PITCH,
            visual_z: gz as f32 * CELL_PITCH,
            input_buffer: if matches!(kind, ModuleKind::RawIntakeHopper | ModuleKind::ProtoBench) {
                4
            } else {
                0
            },
            output_buffer: 0,
            work_progress: 0.0,
            condition: 1.0,
            being_relocated: false,
            level: 1,
            drone_sortie_progress: 0.0,
        }
    }

    pub fn world_pos(&self) -> (f32, f32) {
        (self.gx as f32 * CELL_PITCH, self.gz as f32 * CELL_PITCH)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProductBlueprint {
    pub tier: u8,
    pub name: &'static str,
    pub base_value: f64,
    pub min_stages: u8,
    pub cycle_difficulty: f32,
}

pub const PRODUCT_CATALOG: [ProductBlueprint; 5] = [
    ProductBlueprint {
        tier: 1,
        name: "Servo Actuator Kit",
        base_value: 18.0,
        min_stages: 1,
        cycle_difficulty: 1.0,
    },
    ProductBlueprint {
        tier: 2,
        name: "Autonomous Drone Core",
        base_value: 52.0,
        min_stages: 3,
        cycle_difficulty: 1.35,
    },
    ProductBlueprint {
        tier: 3,
        name: "Quantum LiDAR Array",
        base_value: 135.0,
        min_stages: 4,
        cycle_difficulty: 1.75,
    },
    ProductBlueprint {
        tier: 4,
        name: "Biomimetic Exo-Frame",
        base_value: 310.0,
        min_stages: 5,
        cycle_difficulty: 2.20,
    },
    ProductBlueprint {
        tier: 5,
        name: "Swarm Singularity Core",
        base_value: 740.0,
        min_stages: 5,
        cycle_difficulty: 2.80,
    },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryContract {
    pub id: u32,
    pub title: String,
    pub product_tier: u8,
    pub required_units: u32,
    pub delivered_units: u32,
    pub bonus_credits: f64,
    pub bonus_rp: f64,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PheromoneEdge {
    pub from_id: u32,
    pub to_id: u32,
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub version: String,
    pub tick_count: u64,
    pub elapsed_secs: f64,
    pub credits: f64,
    pub total_revenue: f64,
    pub research_points: f64,
    pub total_products_shipped: u64,
    pub main_floor_half_extent: i32,
    pub active_product_tier: u8,
    pub unlocked_techs: Vec<String>,
    pub robots: Vec<Robot>,
    pub modules: Vec<FactoryModule>,
    pub contracts: Vec<DeliveryContract>,
    pub pheromones: Vec<PheromoneEdge>,
    pub achievements: Vec<String>,
    #[serde(default)]
    pub disasters: Vec<ActiveDisaster>,
    #[serde(default)]
    pub disasters_resolved: u64,
    #[serde(default = "default_tier_one")]
    pub warehouse_flare_tier: u8,
    #[serde(default)]
    pub last_cheer_banner: String,
    pub layout_efficiency: f32,
    pub swarm_coherence: f32,
    pub power_supply_kw: f32,
    pub power_demand_kw: f32,
    pub coolant_pressure_psi: f32,
    pub raw_material_stock: f32,
    pub admin_margin_multiplier: f32,
    pub builder_optimizations_done: u64,
    pub next_entity_id: u32,
    pub event_log: Vec<String>,
    #[serde(skip, default)]
    pub congestion_map: HashMap<(i32, i32), f32>,
    #[serde(skip, default)]
    pub stuck_tracker: HashMap<u32, (f32, f32, f32)>,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new_starter_factory()
    }
}

impl GameState {
    pub fn new_starter_factory() -> Self {
        let mut next_id = 1u32;
        let mut alloc_id = || {
            let id = next_id;
            next_id += 1;
            id
        };

        let modules = vec![
            FactoryModule::new(alloc_id(), ModuleKind::VerticalFreightLift, FloorId::MainFloor, -3, -3),
            FactoryModule::new(alloc_id(), ModuleKind::ProtoBench, FloorId::MainFloor, 3, -3),
            FactoryModule::new(alloc_id(), ModuleKind::DeliveryDock, FloorId::MainFloor, 3, 3),
            FactoryModule::new(alloc_id(), ModuleKind::StorageSilo, FloorId::Basement, -3, 1),
            FactoryModule::new(alloc_id(), ModuleKind::PowerSubstation, FloorId::Basement, 2, -2),
            FactoryModule::new(alloc_id(), ModuleKind::PlumbingChiller, FloorId::Basement, 0, 3),
            FactoryModule::new(alloc_id(), ModuleKind::HydraulicPipingManifold, FloorId::Basement, 3, 2),
            FactoryModule::new(alloc_id(), ModuleKind::SwarmSimMainframe, FloorId::TechResearch, 0, -1),
            FactoryModule::new(alloc_id(), ModuleKind::HoloBoardroomTable, FloorId::Administration, 0, 0),
        ];

        let starter_robot = Robot {
            id: alloc_id(),
            kind: RobotKind::BasicWorker,
            floor: FloorId::MainFloor,
            x: 0.0,
            z: 0.0,
            target_x: 3.0 * CELL_PITCH,
            target_z: -3.0 * CELL_PITCH + 1.60,
            heading: 0.0,
            tier: 1,
            energy: 1.0,
            integrity: 1.0,
            morale_boost_timer: 0.0,
            task: RobotTask::Idle,
            tasks_completed: 0,
        };

        let contracts = vec![
            DeliveryContract {
                id: 1,
                title: "Initial Servo Batch (10 Kits)".to_string(),
                product_tier: 1,
                required_units: 10,
                delivered_units: 0,
                bonus_credits: 120.0,
                bonus_rp: 15.0,
                completed: false,
            },
            DeliveryContract {
                id: 2,
                title: "Regional Automation Supply (35 Kits)".to_string(),
                product_tier: 1,
                required_units: 35,
                delivered_units: 0,
                bonus_credits: 340.0,
                bonus_rp: 40.0,
                completed: false,
            },
            DeliveryContract {
                id: 3,
                title: "Aerospace Drone Core Order (25 Units)".to_string(),
                product_tier: 2,
                required_units: 25,
                delivered_units: 0,
                bonus_credits: 950.0,
                bonus_rp: 85.0,
                completed: false,
            },
        ];

        Self {
            version: "1.3.0".to_string(),
            tick_count: 0,
            elapsed_secs: 0.0,
            credits: 85.0,
            total_revenue: 0.0,
            research_points: 10.0,
            total_products_shipped: 0,
            main_floor_half_extent: 5,
            active_product_tier: 1,
            unlocked_techs: Vec::new(),
            robots: vec![starter_robot],
            modules,
            contracts,
            pheromones: Vec::new(),
            achievements: Vec::new(),
            disasters: Vec::new(),
            disasters_resolved: 0,
            warehouse_flare_tier: 1,
            last_cheer_banner: String::new(),
            layout_efficiency: 0.56,
            swarm_coherence: 1.0,
            power_supply_kw: 45.0,
            power_demand_kw: 26.5,
            coolant_pressure_psi: 88.0,
            raw_material_stock: 120.0,
            admin_margin_multiplier: 1.0,
            builder_optimizations_done: 0,
            next_entity_id: next_id,
            event_log: vec![
                "Facility online: 1 Mk-I Basic Bot active on 10x10 Floor 1.".to_string(),
                "Disasters, Fire Suppression, Repair Bays, CheerBots & Drone Sorties enabled.".to_string(),
            ],
            congestion_map: HashMap::new(),
            stuck_tracker: HashMap::new(),
        }
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }

    pub fn push_log(&mut self, msg: impl Into<String>) {
        self.event_log.push(msg.into());
        if self.event_log.len() > 40 {
            let excess = self.event_log.len() - 40;
            self.event_log.drain(0..excess);
        }
    }

    pub fn has_tech(&self, id: &str) -> bool {
        self.unlocked_techs.iter().any(|t| t == id)
    }
}
