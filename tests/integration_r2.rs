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

use egui::Shape;
use swarmforge::app::SwarmForgeApp;
use swarmforge::assembly::tick_factory;
use swarmforge::floors::purchase_module_at;
use swarmforge::model::{GameState, ModuleKind};
use swarmforge::navigation::apply_orthogonal_unstick;
use swarmforge::persistence::SaveStore;

fn collect_shape_texts(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Text(ts) => out.push(ts.galley.text().to_string()),
        Shape::Vec(items) => {
            for s in items {
                collect_shape_texts(s, out);
            }
        }
        _ => {}
    }
}

fn collect_all_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    let mut out = Vec::new();
    for cs in shapes {
        collect_shape_texts(&cs.shape, &mut out);
    }
    out
}

fn run_ui_frame(ctx: &egui::Context, app: &mut SwarmForgeApp) -> Vec<String> {
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        ..Default::default()
    };
    let output = ctx.run(raw, |c| {
        app.ui_step(c);
    });
    collect_all_texts(&output.shapes)
}

#[test]
fn gui_renders_copyright_power_coolant_single_floor_codes_and_zero_tofu_chars() {
    let mut state = GameState::new_starter_factory();
    for i in 0..45 {
        state.push_log(format!("Integration log line {i}"));
    }

    let ctx = egui::Context::default();
    let mut app = SwarmForgeApp::new_headless(state);

    // Warm up one frame then inspect rendered UI strings
    let _ = run_ui_frame(&ctx, &mut app);
    let texts = run_ui_frame(&ctx, &mut app);
    let joined = texts.join("\n");

    assert!(
        joined.contains("Copyright CRV Science Network"),
        "Missing visible copyright in GUI:\n{joined}"
    );
    assert!(joined.contains("About"), "Missing About button in HUD");
    assert!(joined.contains("Power:"), "Missing Power metric in HUD");
    assert!(joined.contains("Coolant:"), "Missing Coolant metric in HUD");
    assert!(
        joined.contains("B1: Utility Grid, Piping & Storage"),
        "Missing B1 floor title"
    );
    assert!(
        joined.contains("F1: Main Robotic Assembly Floor"),
        "Missing F1 floor title"
    );

    for dup in ["B1 — B1:", "F1 — F1:", "F2 — F2:", "F3 — F3:"] {
        assert!(!joined.contains(dup), "Found duplicated floor prefix '{dup}'");
    }

    let forbidden_tofu = ['₡', '→', '🧯', '⊞', '▶', '⏸', '⚡', '🔥', '↺', '⚠'];
    for ch in forbidden_tofu {
        assert!(
            !joined.contains(ch),
            "Found forbidden tofu-risk glyph '{ch}' in rendered UI text"
        );
    }

    // Open About window and verify version, GPL-3.0-or-later, and clickable links
    app.open_about(true);
    let _ = run_ui_frame(&ctx, &mut app);
    let about_texts = run_ui_frame(&ctx, &mut app).join("\n");
    assert!(about_texts.contains("About SwarmForge"));
    assert!(about_texts.contains("1.3.0"));
    assert!(about_texts.contains("GPL-3.0-or-later"));
    assert!(about_texts.contains("https://crvscience.com"));
    assert!(about_texts.contains("https://crvscience.com/apps"));

    // Expand 40-line event log drawer
    app.set_event_log_expanded(true);
    let _ = run_ui_frame(&ctx, &mut app);
    let log_texts = run_ui_frame(&ctx, &mut app).join("\n");
    assert!(log_texts.contains("Facility Event Log (40/40 entries)"));
    assert!(log_texts.contains("Integration log line 44"));
}

#[test]
fn end_to_end_persistence_manual_placement_and_unstick_simulation() {
    let mut state = GameState::new_starter_factory();
    state.credits = 2500.0;
    let mod_id = purchase_module_at(&mut state, ModuleKind::FabricationCell, -1, 2)
        .expect("manual module placement");

    for _ in 0..30 {
        tick_factory(&mut state, 0.1);
    }

    let (ux, uz) = apply_orthogonal_unstick(0.0, 1.0, 1.0);
    assert!(ux < -2.0 && uz.abs() < 0.5);

    let store = SaveStore::open(":memory:").expect("open in-memory sqlite");
    store.save_slot("e2e_slot", &state).expect("save slot");
    let slots = store.list_slots().expect("list slots");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].slot_name, "e2e_slot");

    let loaded = store.load_slot("e2e_slot").expect("load slot");
    assert!(loaded
        .modules
        .iter()
        .any(|m| m.id == mod_id && m.gx == -1 && m.gz == 2));

    store.delete_slot("e2e_slot").expect("delete slot");
    assert!(store.list_slots().expect("list after delete").is_empty());
}
