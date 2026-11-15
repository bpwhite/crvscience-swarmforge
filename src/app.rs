#![forbid(unsafe_code)]

use crate::assembly::{complete_deliveries, tick_factory};
use crate::builder_ai::count_open_spaced_slots;
use crate::floors::{
    expand_main_floor, expansion_cost, module_current_cost, purchase_module, purchase_robot,
    robot_current_cost,
};
use crate::iso_renderer::{render_isometric_factory, FloorViewMode, IsoCamera};
use crate::model::{FloorId, GameState, ModuleKind, RobotKind, PRODUCT_CATALOG};
use crate::navigation::enforce_spacing_and_migrate;
use crate::persistence::{export_json_file, import_json_file, SaveStore};
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
    slot_input: String,
    json_path_input: String,
    status_banner: String,
}

impl SwarmForgeApp {
    pub fn new(mut state: GameState, db_path: PathBuf) -> Self {
        enforce_spacing_and_migrate(&mut state);
        let save_store = SaveStore::open(&db_path).ok();
        Self {
            state,
            camera: IsoCamera::default(),
            tab: SidebarTab::Robots,
            sim_speed: 1.0,
            paused: false,
            last_frame: Instant::now(),
            save_store,
            slot_input: "slot_1".to_string(),
            json_path_input: "swarmforge_save.json".to_string(),
            status_banner: "Ready — Spacious 2-bay clearance & solid robot collision active.".to_string(),
        }
    }
}

impl eframe::App for SwarmForgeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
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

        ctx.input(|i| {
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
        });

        self.draw_top_hud(ctx);
        self.draw_bottom_log(ctx);
        self.draw_left_command_dock(ctx);
        self.draw_central_viewport(ctx);

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
                    RichText::new(format!("Credits: ₡{:.0}", self.state.credits))
                        .strong()
                        .color(Color32::from_rgb(250, 215, 90)),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("Research: {:.1} RP", self.state.research_points))
                        .color(Color32::from_rgb(115, 225, 255)),
                );
                ui.separator();
                ui.label(format!("Shipped: {}", self.state.total_products_shipped));
                ui.separator();
                ui.label(format!(
                    "Pathing η: {:.0}% ({} opts)",
                    self.state.layout_efficiency * 100.0,
                    self.state.builder_optimizations_done
                ));
                ui.separator();
                ui.label(format!("Swarm SCI: {:.2}x", self.state.swarm_coherence));
                ui.separator();
                if ui
                    .button(if self.paused { "▶ Resume" } else { "⏸ Pause" })
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
                if ui.button("⚡ Pulse Sell").clicked() {
                    complete_deliveries(&mut self.state, 1);
                }
                if !self.state.disasters.is_empty()
                    && ui
                        .button(
                            RichText::new(format!("🧯 Suppress ({})", self.state.disasters.len()))
                                .color(Color32::from_rgb(255, 130, 95)),
                        )
                        .clicked()
                {
                    crate::disasters::trigger_emergency_suppression(&mut self.state);
                }
                if ui.button("🔥 Drill").on_hover_text("Trigger test hazard drill").clicked() {
                    crate::disasters::spawn_random_disaster(&mut self.state);
                }
                ui.label(format!("Flare T{}", self.state.warehouse_flare_tier));
            });

            ui.horizontal_wrapped(|ui| {
                ui.label("Floor View:");
                for f in FloorId::ALL {
                    let active = self.camera.view_mode == FloorViewMode::Single(f);
                    if ui
                        .selectable_label(active, format!("{} — {}", f.code(), f.title()))
                        .clicked()
                    {
                        self.camera.view_mode = FloorViewMode::Single(f);
                    }
                }
                let cut = self.camera.view_mode == FloorViewMode::AllFloorsCutaway;
                if ui.selectable_label(cut, "0 — 4-Floor Cutaway").clicked() {
                    self.camera.view_mode = FloorViewMode::AllFloorsCutaway;
                }
                ui.separator();
                // Always-visible Floor 1 Expansion button right on the top Floor bar!
                let span = self.state.main_floor_half_extent * 2;
                let open_slots = count_open_spaced_slots(&self.state, FloorId::MainFloor);
                if let Some(cost) = expansion_cost(&self.state) {
                    let next_span = span + 2;
                    if ui
                        .add_enabled(
                            self.state.credits >= cost,
                            egui::Button::new(format!(
                                "⊞ Expand F1 ({span}x{span} → {next_span}x{next_span} · ₡{cost:.0}) [{open_slots} open]"
                            )),
                        )
                        .clicked()
                    {
                        let _ = expand_main_floor(&mut self.state);
                    }
                } else {
                    ui.label(format!("F1 Max 26x26 ({open_slots} open bays)"));
                }
                if ui.button("↺ Rotate 90°").clicked() {
                    self.camera.yaw += std::f32::consts::FRAC_PI_2;
                }
            });
        });
    }

    fn draw_bottom_log(&mut self, ctx: &Context) {
        egui::TopBottomPanel::bottom("bottom_telemetry").show(ctx, |ui| {
            ui.horizontal(|ui| {
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
        });
    }

    fn draw_left_command_dock(&mut self, ctx: &Context) {
        egui::SidePanel::left("command_dock")
            .resizable(true)
            .default_width(370.0)
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
                                egui::Button::new(format!("Buy ₡{cost:.0}")),
                            )
                            .clicked()
                        {
                            if let Err(e) = purchase_robot(&mut self.state, kind) {
                                self.status_banner = e;
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
            ui.small("Strict 2-bay minimum clearance keeps wide 360° corridors around every station.");
            if let Some(cost) = expansion_cost(&self.state) {
                if ui
                    .add_enabled(
                        self.state.credits >= cost,
                        egui::Button::new(format!(
                            "⊞ Expand Floor 1 to {}x{} Bays (₡{cost:.0})",
                            span + 2,
                            span + 2
                        )),
                    )
                    .clicked()
                {
                    let _ = expand_main_floor(&mut self.state);
                }
            } else {
                ui.small("Maximum 26x26 Megafactory slab unlocked.");
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
                        format!("T{}: {} (₡{:.0})", bp.tier, bp.name, bp.base_value),
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let btn_lbl = if has_room {
                            format!("Buy ₡{cost:.0}")
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
                            if let Err(e) = purchase_module(&mut self.state, kind) {
                                self.status_banner = e;
                            }
                        }
                    });
                });
            });
        }
    }

    fn ui_tech_tab(&mut self, ui: &mut egui::Ui) {
        for (branch, title) in [
            (
                TechBranch::SwarmIntelligence,
                "Swarm Intelligence Tree (S1–S8)",
            ),
            (
                TechBranch::RoboticsAndFacility,
                "Robotics & Multi-Floor Tree (R1–R8)",
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
                            } else if ui
                                .add_enabled(
                                    ready,
                                    egui::Button::new(format!(
                                        "{:.0} RP + ₡{:.0}",
                                        node.rp_cost, node.credit_cost
                                    )),
                                )
                                .clicked()
                            {
                                let _ = unlock_tech(&mut self.state, node.id);
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
                    "{}/{} shipped | Bonus: +₡{:.0}, +{:.0} RP",
                    c.delivered_units, c.required_units, c.bonus_credits, c.bonus_rp
                )));
            });
        }
    }

    fn ui_persistence_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("SQLite WAL Slots & JSON Sync");
        ui.horizontal(|ui| {
            ui.label("Slot:");
            ui.text_edit_singleline(&mut self.slot_input);
        });
        ui.horizontal(|ui| {
            if ui.button("Save SQLite Slot").clicked() {
                if let Some(store) = &self.save_store {
                    match store.save_slot(&self.slot_input, &self.state) {
                        Ok(()) => self
                            .state
                            .push_log(format!("Saved slot '{}'.", self.slot_input)),
                        Err(e) => self.state.push_log(e),
                    }
                }
            }
            if ui.button("Load SQLite Slot").clicked() {
                if let Some(store) = &self.save_store {
                    if let Ok(mut loaded) = store.load_slot(&self.slot_input) {
                        enforce_spacing_and_migrate(&mut loaded);
                        self.state = loaded;
                    }
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("JSON:");
            ui.text_edit_singleline(&mut self.json_path_input);
        });
        ui.horizontal(|ui| {
            if ui.button("Export JSON").clicked() {
                let _ = export_json_file(&self.json_path_input, &self.state);
                self.state
                    .push_log(format!("Exported JSON '{}'.", self.json_path_input));
            }
            if ui.button("Import JSON").clicked() {
                if let Ok(mut s) = import_json_file(&self.json_path_input) {
                    enforce_spacing_and_migrate(&mut s);
                    self.state = s;
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

            let painter = ui.painter_at(rect);
            render_isometric_factory(&painter, rect, &self.state, &self.camera);
        });
    }
}
