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

use crate::assembly::{complete_deliveries, tick_factory};
use crate::builder_ai::count_open_spaced_slots;
use crate::floors::{
    can_place_module_at, expand_main_floor, expansion_cost, floor_half_extent, module_current_cost,
    purchase_module, purchase_module_at, purchase_robot, robot_current_cost,
};
use crate::iso_renderer::{
    pick_entity, render_isometric_factory_interactive, FloorViewMode, HitBox, HitEntity, IsoCamera,
};
use crate::model::{
    ActiveDisaster, DisasterKind, FloorId, GameState, ModuleKind, RobotKind, RobotTask,
    PRODUCT_CATALOG,
};
use crate::navigation::enforce_spacing_and_migrate;
use crate::persistence::{export_json_file, import_json_file, SaveSlotSummary, SaveStore};
use crate::swarm_tech::{can_unlock_tech, unlock_tech, TechBranch, TECH_TREE};
use egui::{Color32, Context, RichText, ScrollArea, Sense};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarTab {
    Robots,
    Modules,
    SwarmTech,
    Contracts,
    Persistence,
}

pub struct SwarmForgeApp {
    pub state: GameState,
    camera: IsoCamera,
    tab: SidebarTab,
    sim_speed: f32,
    paused: bool,
    last_frame: Instant,
    save_store: Option<SaveStore>,
    save_store_error: Option<String>,
    slots_cache: Vec<SaveSlotSummary>,
    slot_input: String,
    json_path_input: String,
    status_banner: String,
    persistence_msg: Option<(bool, String)>,
    show_about: bool,
    event_log_expanded: bool,
    last_hits: Vec<HitBox>,
    selected_entity: Option<HitEntity>,
    hovered_entity: Option<HitEntity>,
    placement_mode: Option<ModuleKind>,
    manual_gx: i32,
    manual_gz: i32,
}

impl SwarmForgeApp {
    pub fn new(mut state: GameState, db_path: PathBuf) -> Self {
        enforce_spacing_and_migrate(&mut state);
        let (save_store, save_store_error, status_banner) = match SaveStore::open(&db_path) {
            Ok(store) => (
                Some(store),
                None,
                "Ready - Spacious 2-bay clearance & solid robot collision active.".to_string(),
            ),
            Err(e) => (
                None,
                Some(e.clone()),
                format!("SQLite unavailable: {e}"),
            ),
        };
        let mut app = Self {
            state,
            camera: IsoCamera::default(),
            tab: SidebarTab::Robots,
            sim_speed: 1.0,
            paused: false,
            last_frame: Instant::now(),
            save_store,
            save_store_error,
            slots_cache: Vec::new(),
            slot_input: "slot_1".to_string(),
            json_path_input: "swarmforge_save.json".to_string(),
            status_banner,
            persistence_msg: None,
            show_about: false,
            event_log_expanded: false,
            last_hits: Vec::new(),
            selected_entity: None,
            hovered_entity: None,
            placement_mode: None,
            manual_gx: 0,
            manual_gz: 0,
        };
        app.refresh_slots();
        app
    }

    pub fn new_headless(mut state: GameState) -> Self {
        enforce_spacing_and_migrate(&mut state);
        Self {
            state,
            camera: IsoCamera::default(),
            tab: SidebarTab::Robots,
            sim_speed: 1.0,
            paused: true,
            last_frame: Instant::now(),
            save_store: None,
            save_store_error: None,
            slots_cache: Vec::new(),
            slot_input: "slot_1".to_string(),
            json_path_input: "swarmforge_save.json".to_string(),
            status_banner: "Ready - Spacious 2-bay clearance & solid robot collision active."
                .to_string(),
            persistence_msg: None,
            show_about: false,
            event_log_expanded: false,
            last_hits: Vec::new(),
            selected_entity: None,
            hovered_entity: None,
            placement_mode: None,
            manual_gx: 0,
            manual_gz: 0,
        }
    }

    pub fn open_about(&mut self, open: bool) {
        self.show_about = open;
    }

    pub fn set_event_log_expanded(&mut self, expanded: bool) {
        self.event_log_expanded = expanded;
    }

    fn set_status(&mut self, ok: bool, msg: String) {
        self.status_banner = msg.clone();
        self.persistence_msg = Some((ok, msg));
    }

    fn refresh_slots(&mut self) {
        if let Some(store) = &self.save_store {
            match store.list_slots() {
                Ok(slots) => self.slots_cache = slots,
                Err(e) => self.set_status(false, format!("Failed to list SQLite slots: {e}")),
            }
        } else {
            self.slots_cache.clear();
        }
    }

    fn save_slot_by_name(&mut self, slot_name: &str) {
        let slot = slot_name.trim().to_string();
        let Some(store) = &self.save_store else {
            let err = self
                .save_store_error
                .clone()
                .unwrap_or_else(|| "SQLite save store is unavailable.".to_string());
            self.set_status(false, err);
            return;
        };
        match store.save_slot(&slot, &self.state) {
            Ok(()) => {
                let msg = format!("Saved slot '{slot}'.");
                self.state.push_log(msg.clone());
                self.set_status(true, msg);
                self.refresh_slots();
            }
            Err(e) => {
                self.state.push_log(e.clone());
                self.set_status(false, e);
            }
        }
    }

    fn load_slot_by_name(&mut self, slot_name: &str) {
        let slot = slot_name.trim().to_string();
        let Some(store) = &self.save_store else {
            let err = self
                .save_store_error
                .clone()
                .unwrap_or_else(|| "SQLite save store is unavailable.".to_string());
            self.set_status(false, err);
            return;
        };
        match store.load_slot(&slot) {
            Ok(mut loaded) => {
                enforce_spacing_and_migrate(&mut loaded);
                self.state = loaded;
                self.selected_entity = None;
                self.hovered_entity = None;
                self.placement_mode = None;
                self.slot_input = slot.clone();
                let msg = format!("Loaded slot '{slot}'.");
                self.state.push_log(msg.clone());
                self.set_status(true, msg);
            }
            Err(e) => {
                self.state.push_log(e.clone());
                self.set_status(false, e);
            }
        }
    }

    fn delete_slot_by_name(&mut self, slot_name: &str) {
        let slot = slot_name.trim().to_string();
        let Some(store) = &self.save_store else {
            let err = self
                .save_store_error
                .clone()
                .unwrap_or_else(|| "SQLite save store is unavailable.".to_string());
            self.set_status(false, err);
            return;
        };
        match store.delete_slot(&slot) {
            Ok(()) => {
                let msg = format!("Deleted slot '{slot}'.");
                self.state.push_log(msg.clone());
                self.set_status(true, msg);
                self.refresh_slots();
            }
            Err(e) => {
                self.state.push_log(e.clone());
                self.set_status(false, e);
            }
        }
    }
}

impl eframe::App for SwarmForgeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.ui_step(ctx);
    }
}

impl SwarmForgeApp {
    pub fn ui_step(&mut self, ctx: &Context) {
        let now = Instant::now();
        let raw_dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        if !self.paused {
            let steps = (self.sim_speed.ceil() as usize).max(1);
            let sub_dt = (raw_dt * self.sim_speed) / (steps as f32);
            for _ in 0..steps {
                tick_factory(&mut self.state, sub_dt);
            }
        }

        let wants_kb = ctx.wants_keyboard_input();
        ctx.input(|i| {
            if i.key_pressed(egui::Key::F1) {
                self.show_about = !self.show_about;
            }
            if i.key_pressed(egui::Key::Escape) {
                if self.show_about {
                    self.show_about = false;
                } else if self.placement_mode.is_some() {
                    self.placement_mode = None;
                } else {
                    self.selected_entity = None;
                }
            }
            if !wants_kb {
                if i.key_pressed(egui::Key::Space) {
                    self.paused = !self.paused;
                }
                if i.key_pressed(egui::Key::Num1) {
                    self.camera.view_mode = FloorViewMode::Single(FloorId::Basement);
                }
                if i.key_pressed(egui::Key::Num2) {
                    self.camera.view_mode = FloorViewMode::Single(FloorId::MainFloor);
                }
                if i.key_pressed(egui::Key::Num3) {
                    self.camera.view_mode = FloorViewMode::Single(FloorId::TechResearch);
                }
                if i.key_pressed(egui::Key::Num4) {
                    self.camera.view_mode = FloorViewMode::Single(FloorId::Administration);
                }
                if i.key_pressed(egui::Key::Num0) {
                    self.camera.view_mode = FloorViewMode::AllFloorsCutaway;
                }
                if i.key_pressed(egui::Key::Q) {
                    self.camera.yaw -= std::f32::consts::FRAC_PI_2;
                }
                if i.key_pressed(egui::Key::E) {
                    self.camera.yaw += std::f32::consts::FRAC_PI_2;
                }
            }
        });

        self.draw_top_hud(ctx);
        self.draw_bottom_log(ctx);
        self.draw_left_command_dock(ctx);
        self.draw_central_viewport(ctx);
        self.draw_about_window(ctx);

        ctx.request_repaint();
    }
}

impl SwarmForgeApp {
    fn draw_top_hud(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("top_hud").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("SWARMFORGE 3D v1.3")
                        .strong()
                        .color(Color32::from_rgb(80, 220, 250)),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("Credits: {:.0} Cr", self.state.credits))
                        .strong()
                        .color(Color32::from_rgb(250, 215, 90)),
                )
                .on_hover_text("Available factory capital earned from product deliveries and B2B contracts");
                ui.separator();
                ui.label(
                    RichText::new(format!("Research: {:.1} RP", self.state.research_points))
                        .color(Color32::from_rgb(115, 225, 255)),
                )
                .on_hover_text("Research Points generated by Floor 2 Tech Labs and Lab Research Bots");
                ui.separator();
                ui.label(format!("Shipped: {}", self.state.total_products_shipped))
                    .on_hover_text("Total finished robotic products delivered via Autonomous Delivery Docks");
                ui.separator();
                ui.label(format!(
                    "Pathing Eff: {:.0}% ({} opts)",
                    self.state.layout_efficiency * 100.0,
                    self.state.builder_optimizations_done
                ))
                .on_hover_text("Factory aisle routing efficiency and Builder Architect layout optimizations completed");
                ui.separator();
                ui.label(format!("Swarm SCI: {:.2}x", self.state.swarm_coherence))
                    .on_hover_text("Swarm Coherence Index multiplier from unlocked Swarm Intelligence technologies");
                ui.separator();
                let power_ok = self.state.power_demand_kw <= self.state.power_supply_kw;
                let power_color = if power_ok {
                    Color32::from_rgb(95, 235, 165)
                } else {
                    Color32::from_rgb(255, 115, 85)
                };
                ui.label(
                    RichText::new(format!(
                        "Power: {:.1}/{:.1} kW",
                        self.state.power_demand_kw, self.state.power_supply_kw
                    ))
                    .color(power_color),
                )
                .on_hover_text("Total facility power demand vs. Basement HV substation capacity");
                ui.separator();
                let coolant_color = if self.state.coolant_pressure_psi >= 85.0 {
                    Color32::from_rgb(115, 225, 255)
                } else {
                    Color32::from_rgb(250, 195, 75)
                };
                ui.label(
                    RichText::new(format!(
                        "Coolant: {:.0} psi",
                        self.state.coolant_pressure_psi
                    ))
                    .color(coolant_color),
                )
                .on_hover_text("Closed-loop cryo coolant pressure maintained by Basement chillers and manifolds");
                ui.separator();
                ui.label(format!("Flare T{}", self.state.warehouse_flare_tier))
                    .on_hover_text("Facility scale & logistics flare tier driving delivery volume and drone fleet activity");
                ui.separator();
                if ui.button("About").clicked() {
                    self.show_about = !self.show_about;
                }
            });

            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(if self.paused { "> Resume" } else { "|| Pause" })
                    .clicked()
                {
                    self.paused = !self.paused;
                }
                for (lbl, spd) in [("1x", 1.0), ("2x", 2.0), ("4x", 4.0)] {
                    if ui
                        .selectable_label((self.sim_speed - spd).abs() < 0.1, lbl)
                        .clicked()
                    {
                        self.sim_speed = spd;
                    }
                }
                if ui.button("Pulse Sell").clicked() {
                    complete_deliveries(&mut self.state, 1);
                }
                if !self.state.disasters.is_empty()
                    && ui
                        .button(
                            RichText::new(format!("[!] Suppress ({})", self.state.disasters.len()))
                                .color(Color32::from_rgb(255, 130, 95)),
                        )
                        .clicked()
                {
                    crate::disasters::trigger_emergency_suppression(&mut self.state);
                }
                if ui
                    .button("Drill")
                    .on_hover_text("Trigger test hazard drill")
                    .clicked()
                {
                    crate::disasters::spawn_random_disaster(&mut self.state);
                }
                ui.separator();
                ui.label("Floor View:");
                for f in FloorId::ALL {
                    let active = self.camera.view_mode == FloorViewMode::Single(f);
                    if ui.selectable_label(active, f.title()).clicked() {
                        self.camera.view_mode = FloorViewMode::Single(f);
                    }
                }
                let cut = self.camera.view_mode == FloorViewMode::AllFloorsCutaway;
                if ui.selectable_label(cut, "0: 4-Floor Cutaway").clicked() {
                    self.camera.view_mode = FloorViewMode::AllFloorsCutaway;
                }
                ui.separator();
                let span = self.state.main_floor_half_extent * 2;
                let open_slots = count_open_spaced_slots(&self.state, FloorId::MainFloor);
                if let Some(cost) = expansion_cost(&self.state) {
                    let next_span = span + 2;
                    if ui
                        .add_enabled(
                            self.state.credits >= cost,
                            egui::Button::new(format!(
                                "+ Expand F1 ({span}x{span} -> {next_span}x{next_span} | {cost:.0} Cr) [{open_slots} open]"
                            )),
                        )
                        .clicked()
                    {
                        if let Err(e) = expand_main_floor(&mut self.state) {
                            self.status_banner = e.clone();
                            self.state.push_log(e);
                        }
                    }
                } else {
                    ui.label(format!("F1 Max 26x26 ({open_slots} open bays)"));
                }
                if ui.button("Rotate 90 deg").clicked() {
                    self.camera.yaw += std::f32::consts::FRAC_PI_2;
                }
            });
        });
    }

    fn draw_bottom_log(&mut self, ctx: &Context) {
        egui::TopBottomPanel::bottom("bottom_telemetry").show(ctx, |ui| {
            if self.event_log_expanded {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Facility Event Log ({}/40 entries)",
                            self.state.event_log.len()
                        ))
                        .strong(),
                    );
                    if ui.small_button("Collapse").clicked() {
                        self.event_log_expanded = false;
                    }
                });
                ScrollArea::vertical()
                    .id_salt("event_log_drawer")
                    .max_height(150.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for (idx, entry) in self.state.event_log.iter().enumerate() {
                            let color = if entry.starts_with("ALERT:") || entry.contains("failed") {
                                Color32::from_rgb(255, 130, 95)
                            } else if entry.starts_with("Contract Complete")
                                || entry.starts_with("Milestone")
                            {
                                Color32::from_rgb(95, 235, 145)
                            } else {
                                Color32::from_rgb(205, 215, 228)
                            };
                            ui.label(
                                RichText::new(format!("[{:02}] {entry}", idx + 1))
                                    .small()
                                    .color(color),
                            );
                        }
                    });
                ui.separator();
            }

            ui.horizontal(|ui| {
                let toggle_lbl = if self.event_log_expanded {
                    format!("v Log ({})", self.state.event_log.len())
                } else {
                    format!("^ Log ({})", self.state.event_log.len())
                };
                if ui.small_button(toggle_lbl).clicked() {
                    self.event_log_expanded = !self.event_log_expanded;
                }
                let latest = self
                    .state
                    .event_log
                    .last()
                    .cloned()
                    .unwrap_or_else(|| self.status_banner.clone());
                ui.label(RichText::new("Telemetry:").strong());
                ui.label(latest);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "Robots: {} | Modules: {} | F1 Open Bays: {} | Stock: {:.0}",
                        self.state.robots.len(),
                        self.state.modules.len(),
                        count_open_spaced_slots(&self.state, FloorId::MainFloor),
                        self.state.raw_material_stock
                    ));
                });
            });

            if !self.state.last_cheer_banner.trim().is_empty() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("[Morale] {}", self.state.last_cheer_banner))
                            .small()
                            .strong()
                            .color(Color32::from_rgb(255, 220, 105)),
                    )
                    .on_hover_text(
                        "Latest CheerBot Motivation Drone broadcast (+22% work speed boost to nearby robots)",
                    );
                });
            }

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Status: {}", self.status_banner))
                        .small()
                        .color(Color32::from_rgb(165, 190, 215)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Copyright CRV Science Network")
                            .small()
                            .color(Color32::from_rgb(155, 185, 220)),
                    );
                });
            });
        });
    }

    fn draw_about_window(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        egui::Window::new("About SwarmForge")
            .open(&mut self.show_about)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.heading(format!("SwarmForge v{}", env!("CARGO_PKG_VERSION")));
                ui.label(
                    "3D Isometric Automated Robotic Factory & Swarm Intelligence Simulation",
                );
                ui.separator();
                ui.label(RichText::new("Copyright CRV Science Network").strong());
                ui.label(
                    "Licensed under the GNU General Public License v3.0 or later (GPL-3.0-or-later).",
                );
                ui.small("This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY.");
                ui.add_space(6.0);
                ui.hyperlink_to(
                    "CRV Science Network (https://crvscience.com)",
                    "https://crvscience.com",
                );
                ui.hyperlink_to(
                    "CRV Science Public Apps (https://crvscience.com/apps)",
                    "https://crvscience.com/apps",
                );
                ui.hyperlink_to(
                    "SwarmForge Preprint",
                    "https://crvscience.com/apps/preprints/swarmforge-isometric-swarm-factory-game",
                );
            });
    }

    fn draw_left_command_dock(&mut self, ctx: &Context) {
        egui::SidePanel::left("command_dock")
            .resizable(true)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, SidebarTab::Robots, "Robots");
                    ui.selectable_value(&mut self.tab, SidebarTab::Modules, "Modules");
                    ui.selectable_value(&mut self.tab, SidebarTab::SwarmTech, "Tech Tree");
                    ui.selectable_value(&mut self.tab, SidebarTab::Contracts, "Contracts");
                    ui.selectable_value(&mut self.tab, SidebarTab::Persistence, "Save/DB");
                });
                ui.separator();

                ScrollArea::vertical().show(ui, |ui| match self.tab {
                    SidebarTab::Robots => self.ui_robots_tab(ui),
                    SidebarTab::Modules => self.ui_modules_tab(ui),
                    SidebarTab::SwarmTech => self.ui_tech_tab(ui),
                    SidebarTab::Contracts => self.ui_contracts_tab(ui),
                    SidebarTab::Persistence => self.ui_persistence_tab(ui),
                });
            });
    }

    fn ui_robots_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Autonomous Robot Fleet");
        ui.label("Solid collision & starboard aisle steering active: robots cannot pass through each other or fabs. Builder Bots optimize layouts around real A* traffic corridors.");
        ui.add_space(6.0);

        for kind in [
            RobotKind::BasicWorker,
            RobotKind::Assembler,
            RobotKind::Hauler,
            RobotKind::Builder,
            RobotKind::RepairBot,
            RobotKind::MotivationDrone,
            RobotKind::Maintenance,
            RobotKind::Researcher,
            RobotKind::Administrator,
        ] {
            let count = self.state.robots.iter().filter(|r| r.kind == kind).count() as u32;
            let cost = robot_current_cost(kind, count);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} (x{count})", kind.label())).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                self.state.credits >= cost,
                                egui::Button::new(format!("Buy ({cost:.0} Cr)")),
                            )
                            .clicked()
                        {
                            match purchase_robot(&mut self.state, kind) {
                                Ok(id) => {
                                    self.status_banner =
                                        format!("Commissioned {} #{id}.", kind.label());
                                }
                                Err(e) => self.status_banner = e,
                            }
                        }
                    });
                });
                ui.small(format!("Assigned Floor: {}", kind.home_floor().title()));
            });
        }
    }

    fn ui_modules_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Spaced Floor Modules & F1 Expansion");
        let span = self.state.main_floor_half_extent * 2;
        let open_slots = count_open_spaced_slots(&self.state, FloorId::MainFloor);
        ui.group(|ui| {
            ui.label(
                RichText::new(format!(
                    "Floor 1 Slab: {span}x{span} Bays ({open_slots} spaced slots open)"
                ))
                .strong(),
            );
            ui.small("Strict 2-bay minimum clearance keeps wide 360-deg corridors around every station.");
            if let Some(cost) = expansion_cost(&self.state) {
                if ui
                    .add_enabled(
                        self.state.credits >= cost,
                        egui::Button::new(format!(
                            "+ Expand Floor 1 to {}x{} Bays ({cost:.0} Cr)",
                            span + 2,
                            span + 2
                        )),
                    )
                    .clicked()
                {
                    match expand_main_floor(&mut self.state) {
                        Ok(()) => {
                            let new_span = self.state.main_floor_half_extent * 2;
                            self.status_banner =
                                format!("Expanded Floor 1 to {new_span}x{new_span} bays.");
                        }
                        Err(e) => {
                            self.status_banner = e.clone();
                            self.state.push_log(e);
                        }
                    }
                }
            } else {
                ui.small("Maximum 26x26 Megafactory slab unlocked.");
            }
        });

        ui.separator();
        ui.group(|ui| {
            ui.label(RichText::new("Manual Bay Placement (gx, gz):").strong());
            let max_half = floor_half_extent(&self.state, FloorId::MainFloor);
            ui.horizontal(|ui| {
                ui.label("gx:");
                ui.add(egui::DragValue::new(&mut self.manual_gx).range(-max_half..=(max_half - 1)));
                ui.label("gz:");
                ui.add(egui::DragValue::new(&mut self.manual_gz).range(-max_half..=(max_half - 1)));
                match can_place_module_at(
                    &self.state,
                    FloorId::MainFloor,
                    self.manual_gx,
                    self.manual_gz,
                ) {
                    Ok(()) => {
                        ui.label(
                            RichText::new("F1 Bay Open")
                                .small()
                                .color(Color32::from_rgb(95, 235, 145)),
                        );
                    }
                    Err(_) => {
                        ui.label(
                            RichText::new("F1 Bay Blocked")
                                .small()
                                .color(Color32::from_rgb(255, 130, 95)),
                        );
                    }
                }
            });
            if let Some(armed) = self.placement_mode {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Click {} floor in 3D canvas to place {}",
                            armed.floor().code(),
                            armed.label()
                        ))
                        .small()
                        .color(Color32::from_rgb(80, 220, 250)),
                    );
                    if ui.small_button("Cancel").clicked() {
                        self.placement_mode = None;
                    }
                });
            } else {
                ui.small("Use '@(gx,gz)' for exact coordinates or 'Click' to place on the 3D floor.");
            }
        });

        ui.separator();
        ui.label(RichText::new("Active Product Line:").strong());
        for bp in PRODUCT_CATALOG {
            let unlocked = bp.tier == 1
                || (bp.tier == 2 && self.state.has_tech("robo_modular_line"))
                || (bp.tier == 3 && self.state.has_tech("robo_floor2_labs"))
                || (bp.tier == 4 && self.state.has_tech("robo_bay_expansion"))
                || (bp.tier == 5 && self.state.has_tech("swarm_singularity"));
            if ui
                .add_enabled(
                    unlocked,
                    egui::RadioButton::new(
                        self.state.active_product_tier == bp.tier,
                        format!("T{}: {} ({:.0} Cr)", bp.tier, bp.name, bp.base_value),
                    ),
                )
                .clicked()
            {
                self.state.active_product_tier = bp.tier;
            }
        }

        ui.separator();
        for kind in [
            ModuleKind::ProtoBench,
            ModuleKind::RawIntakeHopper,
            ModuleKind::FabricationCell,
            ModuleKind::SubAssemblyCell,
            ModuleKind::FinalAssemblyCell,
            ModuleKind::QualityPackagingBay,
            ModuleKind::DeliveryDock,
            ModuleKind::BotRepairStation,
            ModuleKind::FireSuppressionTower,
            ModuleKind::StorageSilo,
            ModuleKind::PowerSubstation,
            ModuleKind::PlumbingChiller,
            ModuleKind::HydraulicPipingManifold,
            ModuleKind::MaintenanceBay,
            ModuleKind::SwarmSimMainframe,
            ModuleKind::MaterialsLabRig,
            ModuleKind::HoloBlueprintBench,
            ModuleKind::HoloBoardroomTable,
            ModuleKind::ContractDispatchTerminal,
            ModuleKind::SupplyHedgeServer,
        ] {
            let count = self.state.modules.iter().filter(|m| m.kind == kind).count() as u32;
            let cost = module_current_cost(kind, count);
            let has_room = count_open_spaced_slots(&self.state, kind.floor()) > 0;
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "[{}] {} (x{count})",
                            kind.floor().code(),
                            kind.label()
                        ))
                        .strong(),
                    );
                });
                ui.horizontal(|ui| {
                    let btn_lbl = if has_room {
                        format!("Auto ({cost:.0} Cr)")
                    } else if matches!(
                        kind.floor(),
                        FloorId::Basement | FloorId::TechResearch | FloorId::Administration
                    ) {
                        "Floor Full".to_string()
                    } else {
                        "Expand Floor".to_string()
                    };
                    if ui
                        .add_enabled(
                            has_room && self.state.credits >= cost,
                            egui::Button::new(btn_lbl),
                        )
                        .clicked()
                    {
                        match purchase_module(&mut self.state, kind) {
                            Ok(id) => {
                                self.status_banner = format!("Installed {} #{id}.", kind.label());
                            }
                            Err(e) => self.status_banner = e,
                        }
                    }

                    let manual_ok = can_place_module_at(
                        &self.state,
                        kind.floor(),
                        self.manual_gx,
                        self.manual_gz,
                    )
                    .is_ok()
                        && self.state.credits >= cost;
                    if ui
                        .add_enabled(
                            manual_ok,
                            egui::Button::new(format!(
                                "@({},{})",
                                self.manual_gx, self.manual_gz
                            )),
                        )
                        .on_hover_text("Place directly at manual (gx, gz)")
                        .clicked()
                    {
                        match purchase_module_at(
                            &mut self.state,
                            kind,
                            self.manual_gx,
                            self.manual_gz,
                        ) {
                            Ok(id) => {
                                self.status_banner = format!(
                                    "Placed {} #{id} at ({}, {}).",
                                    kind.label(),
                                    self.manual_gx,
                                    self.manual_gz
                                );
                            }
                            Err(e) => self.status_banner = e,
                        }
                    }

                    let armed = self.placement_mode == Some(kind);
                    if ui
                        .add_enabled(
                            self.state.credits >= cost,
                            egui::SelectableLabel::new(armed, "Click Floor"),
                        )
                        .on_hover_text("Arm click-to-place on the 3D floor")
                        .clicked()
                    {
                        if armed {
                            self.placement_mode = None;
                        } else {
                            self.placement_mode = Some(kind);
                            self.camera.view_mode = FloorViewMode::Single(kind.floor());
                            self.status_banner = format!(
                                "Click {} floor bay in 3D canvas to place {}.",
                                kind.floor().code(),
                                kind.label()
                            );
                        }
                    }
                });
            });
        }
    }

    fn ui_tech_tab(&mut self, ui: &mut egui::Ui) {
        for (branch, title) in [
            (
                TechBranch::SwarmIntelligence,
                "Swarm Intelligence Tree (S1-S8)",
            ),
            (
                TechBranch::RoboticsAndFacility,
                "Robotics & Multi-Floor Tree (R1-R8)",
            ),
        ] {
            ui.heading(title);
            for node in TECH_TREE.iter().filter(|n| n.branch == branch) {
                let owned = self.state.has_tech(node.id);
                let ready = can_unlock_tech(&self.state, node.id);
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let color = if owned {
                            Color32::from_rgb(95, 235, 145)
                        } else {
                            Color32::WHITE
                        };
                        ui.label(
                            RichText::new(format!("[{}] {}", node.code, node.name))
                                .strong()
                                .color(color),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if owned {
                                ui.label(RichText::new("ACTIVE").color(Color32::LIGHT_GREEN));
                            } else {
                                let mut missing_reasons = Vec::new();
                                if let Some(pre_id) = node.prereq {
                                    if !self.state.has_tech(pre_id) {
                                        let pre_label = TECH_TREE
                                            .iter()
                                            .find(|p| p.id == pre_id)
                                            .map(|p| format!("[{}] {}", p.code, p.name))
                                            .unwrap_or_else(|| pre_id.to_string());
                                        missing_reasons.push(format!("Requires {pre_label}"));
                                    }
                                }
                                if self.state.research_points < node.rp_cost {
                                    missing_reasons.push(format!(
                                        "Need {:.1} more RP",
                                        node.rp_cost - self.state.research_points
                                    ));
                                }
                                if self.state.credits < node.credit_cost {
                                    missing_reasons.push(format!(
                                        "Need {:.0} more Cr",
                                        node.credit_cost - self.state.credits
                                    ));
                                }
                                let disabled_tip = missing_reasons.join(" | ");
                                if ui
                                    .add_enabled(
                                        ready,
                                        egui::Button::new(format!(
                                            "{:.0} RP + {:.0} Cr",
                                            node.rp_cost, node.credit_cost
                                        )),
                                    )
                                    .on_disabled_hover_text(disabled_tip)
                                    .clicked()
                                {
                                    match unlock_tech(&mut self.state, node.id) {
                                        Ok(()) => {
                                            self.status_banner = format!(
                                                "Researched [{}] {}.",
                                                node.code, node.name
                                            );
                                        }
                                        Err(e) => {
                                            self.status_banner = e.clone();
                                            self.state.push_log(e);
                                        }
                                    }
                                }
                            }
                        });
                    });
                    ui.small(node.summary);
                });
            }
            ui.add_space(8.0);
        }
    }

    fn ui_contracts_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("B2B Delivery Contracts");
        ui.label("Managed by Floor 3 Executive Robots.");
        for c in &self.state.contracts {
            ui.group(|ui| {
                ui.label(RichText::new(&c.title).strong());
                let frac =
                    (c.delivered_units as f32 / c.required_units.max(1) as f32).clamp(0.0, 1.0);
                ui.add(egui::ProgressBar::new(frac).text(format!(
                    "{}/{} shipped | Bonus: +{:.0} Cr, +{:.0} RP",
                    c.delivered_units, c.required_units, c.bonus_credits, c.bonus_rp
                )));
            });
        }
    }

    fn ui_persistence_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("SQLite WAL Slots & JSON Sync");
        if let Some(err) = &self.save_store_error {
            ui.label(
                RichText::new(format!("SQLite Error: {err}"))
                    .small()
                    .color(Color32::from_rgb(255, 115, 95)),
            );
        }
        if let Some((ok, msg)) = &self.persistence_msg {
            let col = if *ok {
                Color32::from_rgb(95, 235, 145)
            } else {
                Color32::from_rgb(255, 115, 95)
            };
            ui.label(RichText::new(msg).small().color(col));
        }

        ui.horizontal(|ui| {
            ui.label("Slot:");
            ui.text_edit_singleline(&mut self.slot_input);
        });
        ui.horizontal(|ui| {
            if ui.button("Save SQLite Slot").clicked() {
                let name = self.slot_input.clone();
                self.save_slot_by_name(&name);
            }
            if ui.button("Load SQLite Slot").clicked() {
                let name = self.slot_input.clone();
                self.load_slot_by_name(&name);
            }
            if ui.button("Refresh").clicked() {
                self.refresh_slots();
            }
        });

        ui.add_space(6.0);
        ui.label(RichText::new(format!("Saved SQLite Slots ({})", self.slots_cache.len())).strong());
        if self.slots_cache.is_empty() {
            ui.small("No saved SQLite slots yet.");
        } else {
            let slots = self.slots_cache.clone();
            for slot in slots {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&slot.slot_name).strong());
                        ui.small(format!(
                            "{} | {:.0} Cr | {} shipped | {} bots",
                            slot.updated_utc, slot.credits, slot.shipped, slot.robot_count
                        ));
                    });
                    ui.horizontal(|ui| {
                        if ui.small_button("Load").clicked() {
                            self.load_slot_by_name(&slot.slot_name);
                        }
                        if ui.small_button("Overwrite").clicked() {
                            self.save_slot_by_name(&slot.slot_name);
                        }
                        if ui.small_button("Delete").clicked() {
                            self.delete_slot_by_name(&slot.slot_name);
                        }
                    });
                });
            }
        }

        ui.separator();
        ui.label(RichText::new("JSON Snapshot Import / Export").strong());
        ui.small(format!("Last path: {}", self.json_path_input));
        ui.horizontal(|ui| {
            if ui.button("Export JSON...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SwarmForge JSON", &["json"])
                    .set_file_name("swarmforge_save.json")
                    .save_file()
                {
                    match export_json_file(&path, &self.state) {
                        Ok(()) => {
                            self.json_path_input = path.display().to_string();
                            let msg = format!("Exported JSON '{}'.", path.display());
                            self.state.push_log(msg.clone());
                            self.set_status(true, msg);
                        }
                        Err(e) => {
                            let msg = format!("JSON export failed: {e}");
                            self.state.push_log(msg.clone());
                            self.set_status(false, msg);
                        }
                    }
                }
            }
            if ui.button("Import JSON...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SwarmForge JSON", &["json"])
                    .pick_file()
                {
                    match import_json_file(&path) {
                        Ok(mut s) => {
                            enforce_spacing_and_migrate(&mut s);
                            self.state = s;
                            self.selected_entity = None;
                            self.hovered_entity = None;
                            self.placement_mode = None;
                            self.json_path_input = path.display().to_string();
                            let msg = format!("Imported JSON '{}'.", path.display());
                            self.state.push_log(msg.clone());
                            self.set_status(true, msg);
                        }
                        Err(e) => {
                            let msg = format!("JSON import failed: {e}");
                            self.state.push_log(msg.clone());
                            self.set_status(false, msg);
                        }
                    }
                }
            }
        });
    }

    fn draw_central_viewport(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

            if response.dragged_by(egui::PointerButton::Primary) {
                let d = response.drag_delta();
                self.camera.yaw += d.x * 0.008;
                self.camera.pitch = (self.camera.pitch + d.y * 0.006).clamp(0.25, 1.25);
            }
            if response.dragged_by(egui::PointerButton::Secondary) {
                let d = response.drag_delta();
                self.camera.pan_x += d.x;
                self.camera.pan_y += d.y;
            }
            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll.abs() > 0.1 {
                    self.camera.zoom =
                        (self.camera.zoom * (1.0 + scroll * 0.0015)).clamp(6.0, 95.0);
                }
            }

            let hover_pos = response.hover_pos();
            self.hovered_entity = hover_pos.and_then(|p| pick_entity(&self.last_hits, p));

            let active_floor = match self.placement_mode {
                Some(kind) => kind.floor(),
                None => match self.camera.view_mode {
                    FloorViewMode::Single(f) => f,
                    FloorViewMode::AllFloorsCutaway => FloorId::MainFloor,
                },
            };
            let focus_y = self.camera.active_focus_y();
            let floor_y = self.camera.floor_render_y(active_floor);
            let hover_cell = hover_pos.and_then(|p| {
                self.camera
                    .screen_to_floor_grid(p, rect, focus_y, floor_y)
            });

            let ghost_bay = match (self.placement_mode, hover_cell) {
                (Some(kind), Some((gx, gz))) => {
                    let valid = can_place_module_at(&self.state, kind.floor(), gx, gz).is_ok();
                    Some((kind.floor(), gx, gz, valid))
                }
                _ => None,
            };

            if response.clicked_by(egui::PointerButton::Secondary) {
                self.placement_mode = None;
            }

            if response.clicked_by(egui::PointerButton::Primary) {
                if let Some(kind) = self.placement_mode {
                    if let Some((gx, gz)) = hover_cell {
                        match purchase_module_at(&mut self.state, kind, gx, gz) {
                            Ok(id) => {
                                self.status_banner = format!(
                                    "Placed {} #{id} at {} bay ({gx}, {gz}).",
                                    kind.label(),
                                    kind.floor().code()
                                );
                                self.selected_entity = Some(HitEntity::Module(id));
                                self.placement_mode = None;
                            }
                            Err(e) => {
                                self.status_banner = e;
                            }
                        }
                    }
                } else if let Some(pos) = response.interact_pointer_pos() {
                    self.selected_entity = pick_entity(&self.last_hits, pos);
                }
            }

            let painter = ui.painter_at(rect);
            let pick_res = render_isometric_factory_interactive(
                &painter,
                rect,
                &self.state,
                &self.camera,
                self.selected_entity,
                self.hovered_entity,
                ghost_bay,
            );
            self.last_hits = pick_res.entities;

            if let Some(kind) = self.placement_mode {
                if let Some((gx, gz)) = hover_cell {
                    let valid = can_place_module_at(&self.state, kind.floor(), gx, gz).is_ok();
                    response.clone().on_hover_text(format!(
                        "Place {} at {} ({gx}, {gz}) [{}]",
                        kind.label(),
                        kind.floor().code(),
                        if valid { "Valid" } else { "Blocked" }
                    ));
                }
            } else if let Some(ent) = self.hovered_entity {
                if let Some(tip) = self.entity_summary(ent) {
                    response.clone().on_hover_text(tip);
                }
            }

            if let Some(ent) = self.selected_entity {
                self.draw_entity_inspector(ctx, rect, ent);
            }
        });
    }

    fn entity_summary(&self, ent: HitEntity) -> Option<String> {
        match ent {
            HitEntity::Module(id) => self.state.modules.iter().find(|m| m.id == id).map(|m| {
                format!(
                    "{} #{} ({}) bay ({}, {})\nCondition: {:.0}% | In: {} Out: {} | Power: {:.1} kW",
                    m.kind.label(),
                    m.id,
                    m.floor.code(),
                    m.gx,
                    m.gz,
                    m.condition * 100.0,
                    m.input_buffer,
                    m.output_buffer,
                    m.kind.power_draw_kw()
                )
            }),
            HitEntity::Robot(id) => self.state.robots.iter().find(|r| r.id == id).map(|r| {
                format!(
                    "{} #{} ({}) pos ({:.1}, {:.1})\nIntegrity: {:.0}% | Tasks: {} | {:?}",
                    r.kind.label(),
                    r.id,
                    r.floor.code(),
                    r.x,
                    r.z,
                    r.integrity * 100.0,
                    r.tasks_completed,
                    r.task
                )
            }),
        }
    }

    fn draw_entity_inspector(&mut self, ctx: &Context, viewport_rect: egui::Rect, ent: HitEntity) {
        let mut still_open = true;
        let anchor_pos = egui::pos2(viewport_rect.right() - 285.0, viewport_rect.top() + 10.0);
        egui::Area::new(egui::Id::new("entity_inspector_card"))
            .fixed_pos(anchor_pos)
            .show(ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(Color32::from_rgba_unmultiplied(16, 24, 36, 235))
                    .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(75, 130, 185)))
                    .show(ui, |ui| {
                        ui.set_width(265.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("3D Entity Inspector")
                                    .strong()
                                    .color(Color32::from_rgb(80, 220, 250)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Close").clicked() {
                                        still_open = false;
                                    }
                                },
                            );
                        });
                        ui.separator();
                        match ent {
                            HitEntity::Module(id) => {
                                if let Some(m) = self.state.modules.iter().find(|m| m.id == id) {
                                    ui.label(
                                        RichText::new(format!("{} #{}", m.kind.label(), m.id))
                                            .strong(),
                                    );
                                    ui.small(format!("Floor: {}", m.floor.title()));
                                    ui.label(format!("Grid Bay: ({}, {})", m.gx, m.gz));
                                    ui.label(format!("Condition: {:.0}%", m.condition * 100.0));
                                    ui.label(format!(
                                        "Buffers: In {} / Out {}",
                                        m.input_buffer, m.output_buffer
                                    ));
                                    ui.label(format!(
                                        "Progress: {:.0}% | Power: {:.1} kW",
                                        m.work_progress * 100.0,
                                        m.kind.power_draw_kw()
                                    ));
                                    if m.being_relocated {
                                        ui.label(
                                            RichText::new("Status: Builder Relocating")
                                                .color(Color32::from_rgb(255, 215, 75)),
                                        );
                                    }
                                } else {
                                    ui.small("Module no longer exists.");
                                }
                            }
                            HitEntity::Robot(id) => {
                                if let Some(r) = self.state.robots.iter().find(|r| r.id == id) {
                                    ui.label(
                                        RichText::new(format!("{} #{}", r.kind.label(), r.id))
                                            .strong(),
                                    );
                                    ui.small(format!("Floor: {}", r.floor.title()));
                                    ui.label(format!("Position: ({:.2}, {:.2})", r.x, r.z));
                                    ui.label(format!(
                                        "Integrity: {:.0}% | Energy: {:.0}%",
                                        r.integrity * 100.0,
                                        r.energy * 100.0
                                    ));
                                    ui.label(format!("Tasks Completed: {}", r.tasks_completed));
                                    if r.morale_boost_timer > 0.0 {
                                        let morale_label = if r.kind == RobotKind::MotivationDrone
                                        {
                                            format!(
                                                "Cheer Pulse Timer: {:.1}s",
                                                r.morale_boost_timer
                                            )
                                        } else {
                                            format!(
                                                "Morale Boost: +22% ({:.1}s)",
                                                r.morale_boost_timer
                                            )
                                        };
                                        ui.label(
                                            RichText::new(morale_label)
                                                .color(Color32::from_rgb(255, 220, 105)),
                                        );
                                    }
                                    let task_desc = match &r.task {
                                        RobotTask::Idle => "Idle".to_string(),
                                        RobotTask::Operating { module_id, .. } => {
                                            format!("Operating Module #{module_id}")
                                        }
                                        RobotTask::CarryingPart {
                                            stage,
                                            target_module_id,
                                            ..
                                        } => format!(
                                            "Carrying Stage {stage} -> #{target_module_id}"
                                        ),
                                        RobotTask::MovingToRelocate {
                                            module_id,
                                            dest_gx,
                                            dest_gz,
                                        } => format!(
                                            "Moving Mod #{module_id} -> ({dest_gx}, {dest_gz})"
                                        ),
                                        RobotTask::RelocatingModule {
                                            module_id,
                                            dest_gx,
                                            dest_gz,
                                            ..
                                        } => format!(
                                            "Relocating #{module_id} -> ({dest_gx}, {dest_gz})"
                                        ),
                                        RobotTask::SeekingMaintenance { station_id } => {
                                            format!("Seeking Repair Bay #{station_id}")
                                        }
                                    };
                                    ui.label(format!("Task: {task_desc}"));
                                } else {
                                    ui.small("Robot no longer exists.");
                                }
                            }
                        }
                    });
            });
        if !still_open {
            self.selected_entity = None;
        }
    }
}

fn block_on_future<F: std::future::Future>(fut: F) -> F::Output {
    struct NoopWake;
    impl std::task::Wake for NoopWake {
        fn wake(self: std::sync::Arc<Self>) {}
    }
    let waker = std::task::Waker::from(std::sync::Arc::new(NoopWake));
    let mut cx = std::task::Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(val) => return val,
            std::task::Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn write_rgba_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file = std::fs::File::create(path)?;
    let w = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Best);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}

/// Render an offscreen 1600x1000 RGBA PNG screenshot without opening any OS GUI window.
pub fn render_headless_screenshot(
    state: GameState,
    _db_path: &std::path::Path,
    out_path: &std::path::Path,
    view: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    use eframe::egui_wgpu::{wgpu, Renderer, ScreenDescriptor};

    let width: u32 = 1600;
    let height: u32 = 1000;
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = block_on_future(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .ok_or("No GPU adapter available for offscreen rendering")?;
    let (device, queue) = block_on_future(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("swarmforge-offscreen-device"),
            ..Default::default()
        },
        None,
    ))?;

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("swarmforge-offscreen-target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bytes_per_row = width * 4;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("swarmforge-offscreen-staging"),
        size: (bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut renderer = Renderer::new(&device, format, None, 1, false);
    let egui_ctx = Context::default();
    egui_ctx.set_pixels_per_point(1.0);

    let mut app = SwarmForgeApp::new_headless(state);

    if view == 1 {
        // View 1: Default 3D Isometric Factory & Swarm Robots view
        app.tab = SidebarTab::Robots;
        app.camera.view_mode = FloorViewMode::Single(FloorId::MainFloor);
        for _ in 0..120 {
            tick_factory(&mut app.state, 0.1);
        }
    } else if view == 2 {
        // View 2: Multi-floor / Tech-Tree / Swarm Scaling view with translucent cutaway
        app.tab = SidebarTab::SwarmTech;
        app.camera.view_mode = FloorViewMode::AllFloorsCutaway;
        app.camera.zoom = 24.0;
        app.state.credits += 5000.0;
        app.state.research_points += 500.0;
        for node in TECH_TREE.iter().take(4) {
            if let Err(e) = unlock_tech(&mut app.state, node.id) {
                app.status_banner = e;
            }
        }
        for kind in [
            RobotKind::Assembler,
            RobotKind::Hauler,
            RobotKind::Researcher,
            RobotKind::Maintenance,
        ] {
            if let Err(e) = purchase_robot(&mut app.state, kind) {
                app.status_banner = e;
            }
        }
        for _ in 0..80 {
            tick_factory(&mut app.state, 0.1);
        }
    } else if view == 3 {
        // View 3: MainFloor Fire Suppression, RepairBot, CheerBot & 3D Inspector view
        app.tab = SidebarTab::Modules;
        app.camera.view_mode = FloorViewMode::Single(FloorId::MainFloor);
        app.state.credits += 5000.0;
        let tower_id = purchase_module_at(&mut app.state, ModuleKind::FireSuppressionTower, 1, -3)
            .or_else(|_| purchase_module(&mut app.state, ModuleKind::FireSuppressionTower))
            .ok();
        if let Err(e) = purchase_robot(&mut app.state, RobotKind::RepairBot) {
            app.status_banner = e;
        }
        if let Err(e) = purchase_robot(&mut app.state, RobotKind::MotivationDrone) {
            app.status_banner = e;
        }
        for _ in 0..40 {
            tick_factory(&mut app.state, 0.1);
        }
        if let Some(proto) = app
            .state
            .modules
            .iter()
            .find(|m| m.floor == FloorId::MainFloor && m.kind == ModuleKind::ProtoBench)
            .cloned()
        {
            let did = app.state.alloc_id();
            app.state.disasters.push(ActiveDisaster {
                id: did,
                kind: DisasterKind::ThermalFire,
                floor: FloorId::MainFloor,
                module_id: proto.id,
                x: proto.visual_x,
                z: proto.visual_z,
                severity: 0.85,
                suppressed_by_tower: false,
            });
            app.state.push_log(format!(
                "ALERT: Thermal Station Fire erupted at {} on F1!",
                proto.kind.label()
            ));
        } else {
            crate::disasters::spawn_random_disaster(&mut app.state);
        }
        for _ in 0..15 {
            tick_factory(&mut app.state, 0.1);
        }
        app.selected_entity = tower_id.map(HitEntity::Module);
    }

    let warmup_delays_ms = [50u64, 30, 20, 20];
    for (frame_idx, delay_ms) in warmup_delays_ms.into_iter().enumerate() {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width as f32, height as f32),
            )),
            viewports: std::iter::once((
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    native_pixels_per_point: Some(1.0),
                    ..Default::default()
                },
            ))
            .collect(),
            time: Some(frame_idx as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let full_output = egui_ctx.run(raw_input, |ctx| {
            app.ui_step(ctx);
        });
        for (id, image_delta) in &full_output.textures_delta.set {
            renderer.update_texture(&device, &queue, *id, image_delta);
        }
        let paint_jobs = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: 1.0,
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("swarmforge-offscreen-encoder"),
        });
        let user_cmd_bufs = renderer.update_buffers(
            &device,
            &queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("swarmforge-offscreen-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            let mut render_pass = render_pass.forget_lifetime();
            renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }
        if frame_idx == 3 {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        queue.submit(user_cmd_bufs.into_iter().chain(std::iter::once(encoder.finish())));
        device.poll(wgpu::Maintain::Wait);
        for id in &full_output.textures_delta.free {
            renderer.free_texture(id);
        }
    }

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        if let Err(e) = tx.send(res) {
            eprintln!("map_async send error: {e}");
        }
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()??;
    let mapped = slice.get_mapped_range();
    write_rgba_png(out_path, width, height, &mapped)?;
    drop(mapped);
    staging.unmap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_frame_with_events(
        ctx: &Context,
        app: &mut SwarmForgeApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        ctx.run(raw, |c| {
            app.ui_step(c);
        })
    }

    fn key_press(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn collect_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(ts) => out.push(ts.galley.text().to_string()),
                egui::Shape::Vec(items) => {
                    for s in items {
                        walk(s, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in shapes {
            walk(&cs.shape, &mut out);
        }
        out
    }

    fn find_text_pos(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<egui::Pos2> {
        fn walk(shape: &egui::Shape, needle: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(ts) => {
                    if ts.galley.text() == needle {
                        Some(ts.pos)
                    } else {
                        None
                    }
                }
                egui::Shape::Vec(items) => items.iter().find_map(|s| walk(s, needle)),
                _ => None,
            }
        }
        shapes.iter().find_map(|cs| walk(&cs.shape, needle))
    }

    #[test]
    fn hotkeys_f1_escape_space_and_text_focus_guard() {
        let state = GameState::new_starter_factory();
        let first_mod_id = state.modules[0].id;
        let ctx = Context::default();
        let mut app = SwarmForgeApp::new_headless(state);

        // Warm up frame
        let _ = run_frame_with_events(&ctx, &mut app, vec![]);
        assert!(!app.show_about);
        assert!(app.paused);

        // F1 toggles About modal open and closed
        let _ = run_frame_with_events(&ctx, &mut app, vec![key_press(egui::Key::F1)]);
        assert!(app.show_about);
        let _ = run_frame_with_events(&ctx, &mut app, vec![key_press(egui::Key::F1)]);
        assert!(!app.show_about);

        // Layered Escape priority: show_about -> placement_mode -> selected_entity
        app.show_about = true;
        app.placement_mode = Some(ModuleKind::FabricationCell);
        app.selected_entity = Some(HitEntity::Module(first_mod_id));

        let _ = run_frame_with_events(&ctx, &mut app, vec![key_press(egui::Key::Escape)]);
        assert!(!app.show_about);
        assert_eq!(app.placement_mode, Some(ModuleKind::FabricationCell));
        assert_eq!(app.selected_entity, Some(HitEntity::Module(first_mod_id)));

        let _ = run_frame_with_events(&ctx, &mut app, vec![key_press(egui::Key::Escape)]);
        assert!(app.placement_mode.is_none());
        assert_eq!(app.selected_entity, Some(HitEntity::Module(first_mod_id)));

        let _ = run_frame_with_events(&ctx, &mut app, vec![key_press(egui::Key::Escape)]);
        assert!(app.selected_entity.is_none());

        // Space toggles paused when not typing in a text box
        let _ = run_frame_with_events(&ctx, &mut app, vec![key_press(egui::Key::Space)]);
        assert!(!app.paused);
        let _ = run_frame_with_events(&ctx, &mut app, vec![key_press(egui::Key::Space)]);
        assert!(app.paused);

        // Switch to Persistence tab and click the slot_input TextEdit ("slot_1")
        app.tab = SidebarTab::Persistence;
        let out = run_frame_with_events(&ctx, &mut app, vec![]);
        let slot_pos = find_text_pos(&out.shapes, "slot_1")
            .expect("slot_1 text edit must render on Persistence tab");
        let click_pt = slot_pos + egui::vec2(6.0, 6.0);
        let _ = run_frame_with_events(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(click_pt),
                egui::Event::PointerButton {
                    pos: click_pt,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: click_pt,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(
            ctx.wants_keyboard_input(),
            "TextEdit should hold keyboard focus after click"
        );

        let initial_paused = app.paused;
        let initial_yaw = app.camera.yaw;
        let initial_view = app.camera.view_mode;

        let _ = run_frame_with_events(
            &ctx,
            &mut app,
            vec![
                key_press(egui::Key::Space),
                key_press(egui::Key::Num1),
                key_press(egui::Key::Q),
                key_press(egui::Key::E),
            ],
        );
        assert_eq!(app.paused, initial_paused);
        assert_eq!(app.camera.view_mode, initial_view);
        assert!((app.camera.yaw - initial_yaw).abs() < 1e-6);
    }

    #[test]
    fn renders_last_cheer_banner_and_robot_morale_boost_timer() {
        let mut state = GameState::new_starter_factory();
        state.last_cheer_banner = "CheerBot #9: \"Keep forging!\"".to_string();
        let robot_id = state.robots[0].id;
        state.robots[0].morale_boost_timer = 4.2;

        let ctx = Context::default();
        let mut app = SwarmForgeApp::new_headless(state);
        app.selected_entity = Some(HitEntity::Robot(robot_id));

        let _ = run_frame_with_events(&ctx, &mut app, vec![]);
        let out = run_frame_with_events(&ctx, &mut app, vec![]);
        let joined = collect_texts(&out.shapes).join("\n");

        assert!(
            joined.contains("CheerBot #9: \"Keep forging!\""),
            "Expected last_cheer_banner in bottom log:\n{joined}"
        );
        assert!(
            joined.contains("Morale Boost: +22% (4.2s)"),
            "Expected robot morale_boost_timer in entity inspector:\n{joined}"
        );

        app.state.last_cheer_banner.clear();
        let _ = run_frame_with_events(&ctx, &mut app, vec![]);
        let out_cleared = run_frame_with_events(&ctx, &mut app, vec![]);
        let joined_cleared = collect_texts(&out_cleared.shapes).join("\n");
        assert!(!joined_cleared.contains("[Morale]"));
    }
}
