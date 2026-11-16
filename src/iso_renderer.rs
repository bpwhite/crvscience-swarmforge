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

use crate::model::{FloorId, GameState, Robot, RobotTask};
use crate::models_3d::{build_module_parts, build_robot_parts, BoxPart};
use crate::navigation::CELL_PITCH;
use egui::{Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke};
use glam::{Mat4, Quat, Vec3, Vec4};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorViewMode {
    Single(FloorId),
    AllFloorsCutaway,
}

#[derive(Debug, Clone)]
pub struct IsoCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub view_mode: FloorViewMode,
}

impl Default for IsoCamera {
    fn default() -> Self {
        Self {
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: 0.615_479_7,
            zoom: 20.0,
            pan_x: 0.0,
            pan_y: 0.0,
            view_mode: FloorViewMode::Single(FloorId::MainFloor),
        }
    }
}

#[derive(Clone)]
struct ProjectedQuad {
    pts: [Pos2; 4],
    depth: f32,
    color: Color32,
}

impl IsoCamera {
    fn view_matrix(&self, target_y: f32) -> Mat4 {
        let dist = 58.0;
        let cp = self.pitch.cos();
        let eye = Vec3::new(
            self.yaw.cos() * cp * dist,
            target_y + self.pitch.sin() * dist,
            self.yaw.sin() * cp * dist,
        );
        Mat4::look_at_rh(eye, Vec3::new(0.0, target_y, 0.0), Vec3::Y)
    }

    pub fn project(&self, p: Vec3, rect: Rect, target_y: f32) -> (Pos2, f32) {
        let v = self.view_matrix(target_y) * Vec4::new(p.x, p.y, p.z, 1.0);
        let center = rect.center();
        let sx = center.x + self.pan_x + v.x * self.zoom;
        let sy = center.y + self.pan_y - v.y * self.zoom;
        (Pos2::new(sx, sy), v.z)
    }
}

fn push_subdivided_axis_box(
    parts: &mut Vec<BoxPart>,
    center: Vec3,
    size: Vec3,
    rgb: [u8; 3],
    max_segment_len: f32,
) {
    let step = max_segment_len.max(0.5);
    if size.x > step && size.x >= size.z {
        let count = (size.x / step).ceil().max(1.0) as usize;
        let seg_x = size.x / (count as f32);
        let start_x = center.x - 0.5 * size.x + 0.5 * seg_x;
        for i in 0..count {
            parts.push(BoxPart::axis_aligned(
                Vec3::new(start_x + (i as f32) * seg_x, center.y, center.z),
                Vec3::new(seg_x, size.y, size.z),
                rgb,
            ));
        }
    } else if size.z > step {
        let count = (size.z / step).ceil().max(1.0) as usize;
        let seg_z = size.z / (count as f32);
        let start_z = center.z - 0.5 * size.z + 0.5 * seg_z;
        for i in 0..count {
            parts.push(BoxPart::axis_aligned(
                Vec3::new(center.x, center.y, start_z + (i as f32) * seg_z),
                Vec3::new(size.x, size.y, seg_z),
                rgb,
            ));
        }
    } else {
        parts.push(BoxPart::axis_aligned(center, size, rgb));
    }
}

fn emit_parts_mesh(
    painter: &Painter,
    cam: &IsoCamera,
    view: &Mat4,
    sun_dir: Vec3,
    rect: Rect,
    focus_y: f32,
    parts: &[BoxPart],
) {
    if parts.is_empty() {
        return;
    }
    let mut quads: Vec<ProjectedQuad> = Vec::with_capacity(parts.len() * 3);
    for part in parts {
        emit_oriented_box(&mut quads, cam, view, sun_dir, rect, focus_y, part);
    }
    quads.sort_by(|a, b| {
        a.depth
            .partial_cmp(&b.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut mesh = Mesh::default();
    for q in &quads {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(q.pts[0], q.color);
        mesh.colored_vertex(q.pts[1], q.color);
        mesh.colored_vertex(q.pts[2], q.color);
        mesh.colored_vertex(q.pts[3], q.color);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(Shape::Mesh(Arc::new(mesh)));
}

pub fn render_isometric_factory(
    painter: &Painter,
    rect: Rect,
    state: &GameState,
    cam: &IsoCamera,
) {
    painter.rect_filled(rect, 0.0, Color32::from_rgb(10, 14, 22));

    let focus_y = match cam.view_mode {
        FloorViewMode::Single(f) => f.world_y(),
        FloorViewMode::AllFloorsCutaway => 2.8,
    };
    let view = cam.view_matrix(focus_y);
    let sun_dir = Vec3::new(0.48, 0.76, 0.43).normalize();
    let time_s = state.elapsed_secs as f32;

    for floor in FloorId::ALL {
        if let FloorViewMode::Single(sel) = cam.view_mode {
            if sel != floor {
                continue;
            }
        }
        let fy = match cam.view_mode {
            FloorViewMode::Single(_) => floor.world_y(),
            FloorViewMode::AllFloorsCutaway => floor.world_y() * 1.28,
        };

        let half_cells = match floor {
            FloorId::MainFloor => state.main_floor_half_extent,
            FloorId::Administration => 4,
            _ => 5,
        };
        let half = half_cells as f32 * CELL_PITCH + 1.0;

        let slab_rgb = match floor {
            FloorId::Basement => [26, 33, 44],
            FloorId::MainFloor => [32, 42, 56],
            FloorId::TechResearch => [27, 46, 60],
            FloorId::Administration => [40, 38, 54],
        };

        // Pass A: Floor slab Shape::Mesh only (guaranteed behind Pass B & Pass C on this floor).
        let slab_part = [BoxPart::axis_aligned(
            Vec3::new(-0.5 * CELL_PITCH, fy - 0.18, -0.5 * CELL_PITCH),
            Vec3::new(half * 2.0, 0.36, half * 2.0),
            slab_rgb,
        )];
        emit_parts_mesh(painter, cam, &view, sun_dir, rect, focus_y, &slab_part);

        // Pass B: Floor tile grid lines & pheromone trails (above slab, strictly below entities).
        let grid_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(90, 125, 160, 42));
        let min_c = (-half_cells as f32 - 0.5) * CELL_PITCH;
        let max_c = (half_cells as f32 - 0.5) * CELL_PITCH;
        for i in -half_cells..=half_cells {
            let coord = (i as f32 - 0.5) * CELL_PITCH;
            let (a1, _) = cam.project(Vec3::new(coord, fy + 0.01, min_c), rect, focus_y);
            let (a2, _) = cam.project(Vec3::new(coord, fy + 0.01, max_c), rect, focus_y);
            painter.line_segment([a1, a2], grid_stroke);
            let (b1, _) = cam.project(Vec3::new(min_c, fy + 0.01, coord), rect, focus_y);
            let (b2, _) = cam.project(Vec3::new(max_c, fy + 0.01, coord), rect, focus_y);
            painter.line_segment([b1, b2], grid_stroke);
        }

        if floor == FloorId::MainFloor {
            for edge in &state.pheromones {
                let src = state.modules.iter().find(|m| m.id == edge.from_id);
                let dst = state.modules.iter().find(|m| m.id == edge.to_id);
                if let (Some(s), Some(d)) = (src, dst) {
                    let (p1, _) =
                        cam.project(Vec3::new(s.visual_x, fy + 0.12, s.visual_z), rect, focus_y);
                    let (p2, _) =
                        cam.project(Vec3::new(d.visual_x, fy + 0.12, d.visual_z), rect, focus_y);
                    let alpha = (edge.intensity * 190.0) as u8;
                    painter.line_segment(
                        [p1, p2],
                        Stroke::new(2.2_f32, Color32::from_rgba_unmultiplied(40, 235, 220, alpha)),
                    );
                }
            }
        }

        // Pass C: All above-floor 3D boxes (robots, modules, shafts, drones, disasters, bollards)
        // with long spanning basement pipes & Tier-4 busbars subdivided into ~2.0u segments.
        let mut parts: Vec<BoxPart> = Vec::with_capacity(1024);
        let mut entity_overlays: Vec<(Pos2, Pos2, Stroke)> = Vec::new();
        let mut badges: Vec<(Pos2, String, Color32)> = Vec::new();

        if floor == FloorId::Basement {
            let pipe_span = half * 1.65;
            push_subdivided_axis_box(
                &mut parts,
                Vec3::new(0.0, fy + 0.14, -4.5 * CELL_PITCH),
                Vec3::new(pipe_span, 0.20, 0.22),
                [217, 119, 6],
                2.0,
            );
            push_subdivided_axis_box(
                &mut parts,
                Vec3::new(-4.5 * CELL_PITCH, fy + 0.14, 0.0),
                Vec3::new(0.22, 0.20, pipe_span),
                [14, 165, 233],
                2.0,
            );
            push_subdivided_axis_box(
                &mut parts,
                Vec3::new(3.5 * CELL_PITCH, fy + 0.14, 0.0),
                Vec3::new(0.22, 0.20, pipe_span),
                [16, 185, 129],
                2.0,
            );
        }

        // Vertical Freight & Utility Shaft anchored cleanly at grid (-3, -3)
        let sx = -3.0 * CELL_PITCH;
        let sz = -3.0 * CELL_PITCH;
        for (px, pz) in [
            (sx - 0.65, sz - 0.65),
            (sx + 0.65, sz - 0.65),
            (sx - 0.65, sz + 0.65),
            (sx + 0.65, sz + 0.65),
        ] {
            parts.push(BoxPart::axis_aligned(
                Vec3::new(px, fy + 1.35, pz),
                Vec3::new(0.16, 2.70, 0.16),
                [71, 85, 105],
            ));
        }
        let car_y = fy + 0.35 + ((time_s * 1.4).sin() * 0.5 + 0.5) * 1.85;
        parts.push(BoxPart::axis_aligned(
            Vec3::new(sx, car_y, sz),
            Vec3::new(1.14, 0.18, 1.14),
            [56, 189, 248],
        ));

        let (lbl_pos, _) = cam.project(Vec3::new(-half, fy + 0.5, half), rect, focus_y);
        badges.push((
            lbl_pos,
            floor.title().to_string(),
            Color32::from_rgb(155, 205, 255),
        ));

        // Non-Obstructing Progressive Perimeter Warehouse Flare (Tiers 1..=6, outside walkable slab!)
        if floor == FloorId::MainFloor {
            let rim = half + 0.55;
            let tier = state.warehouse_flare_tier;
            // Tier 1+: Corner safety bollards
            for &(cx, cz) in &[
                (-rim, -rim),
                (rim - 1.2, -rim),
                (-rim, rim - 1.2),
                (rim - 1.2, rim - 1.2),
            ] {
                parts.push(BoxPart::axis_aligned(
                    Vec3::new(cx, fy + 0.45, cz),
                    Vec3::new(0.24, 0.90, 0.24),
                    [245, 158, 11],
                ));
                if tier >= 2 {
                    // Floodlight stanchions
                    parts.push(BoxPart::axis_aligned(
                        Vec3::new(cx, fy + 1.85, cz),
                        Vec3::new(0.14, 2.20, 0.14),
                        [100, 116, 139],
                    ));
                    parts.push(BoxPart::axis_aligned(
                        Vec3::new(cx, fy + 2.95, cz),
                        Vec3::new(0.48, 0.18, 0.48),
                        [253, 224, 71],
                    ));
                }
                if tier >= 5 {
                    // Spinning holographic corner pylons
                    parts.push(BoxPart::oriented(
                        Vec3::new(cx, fy + 3.65, cz),
                        Vec3::new(0.52, 0.52, 0.52),
                        Quat::from_rotation_y(time_s * 1.5),
                        [56, 235, 255],
                    ));
                }
            }
            if tier >= 3 {
                // Digital Scoreboard Gantry on back perimeter (-rim)
                push_subdivided_axis_box(
                    &mut parts,
                    Vec3::new(0.0, fy + 2.40, -rim),
                    Vec3::new(4.6, 0.85, 0.18),
                    [15, 23, 42],
                    2.0,
                );
                push_subdivided_axis_box(
                    &mut parts,
                    Vec3::new(0.0, fy + 2.40, -rim + 0.10),
                    Vec3::new(4.2, 0.55, 0.05),
                    [34, 211, 238],
                    2.0,
                );
            }
            if tier >= 4 {
                // High-clearance overhead cyan gantry busbar truss (y = 4.1u, completely above bots & fabs!)
                push_subdivided_axis_box(
                    &mut parts,
                    Vec3::new(-0.5 * CELL_PITCH, fy + 4.10, 0.0),
                    Vec3::new(half * 1.85, 0.12, 0.18),
                    [56, 189, 248],
                    2.0,
                );
            }
            if tier >= 6 {
                // Golden Swarm Monument on front perimeter
                parts.push(BoxPart::oriented(
                    Vec3::new(0.0, fy + 1.65, rim - 0.6),
                    Vec3::new(0.68, 0.68, 0.68),
                    Quat::from_rotation_y(time_s * 1.1) * Quat::from_rotation_x(0.45),
                    [250, 204, 21],
                ));
            }
        }

        // Autonomous Non-Obstructing Indoor Ambient Micro-Drones (visual flare cruising at fy + 3.4u!)
        let drone_count = (state.warehouse_flare_tier as usize).clamp(1, 5);
        if floor == FloorId::MainFloor {
            for di in 0..drone_count {
                let ang = time_s * (0.55 + di as f32 * 0.12) + (di as f32 * 1.57);
                let rx = (ang.cos() * (half * 0.62)) - 0.5 * CELL_PITCH;
                let rz = ((ang * 2.0).sin() * (half * 0.42)) - 0.5 * CELL_PITCH;
                let dy = fy + 3.35 + (time_s * 2.5 + di as f32).sin() * 0.15;
                parts.push(BoxPart::axis_aligned(
                    Vec3::new(rx, dy, rz),
                    Vec3::new(0.32, 0.10, 0.32),
                    [148, 163, 184],
                ));
                parts.push(BoxPart::axis_aligned(
                    Vec3::new(rx, dy - 0.08, rz),
                    Vec3::new(0.12, 0.08, 0.12),
                    [56, 235, 255],
                ));
            }
        }

        // Render Active Disasters & Fire Suppression Foam Beams on this floor
        for d in state.disasters.iter().filter(|d| d.floor == floor) {
            let flicker = ((time_s * 12.0 + d.id as f32).sin() * 0.18).abs();
            let col = match d.kind {
                crate::model::DisasterKind::ThermalFire => [249, 115, 22],
                crate::model::DisasterKind::GridOverload => [168, 85, 247],
                crate::model::DisasterKind::CoolantLeak => [34, 211, 238],
                crate::model::DisasterKind::LogicGlitch => [236, 72, 153],
            };
            parts.push(BoxPart::oriented(
                Vec3::new(d.x, fy + 1.65 + flicker, d.z),
                Vec3::new(
                    0.55 * d.severity.max(0.4),
                    0.78 + flicker,
                    0.55 * d.severity.max(0.4),
                ),
                Quat::from_rotation_y(time_s * 5.0),
                col,
            ));
            let (dp, _) = cam.project(Vec3::new(d.x, fy + 2.45, d.z), rect, focus_y);
            badges.push((
                dp,
                format!("⚠ {}", d.kind.label()),
                Color32::from_rgb(255, 110, 75),
            ));

            if d.suppressed_by_tower {
                for tm in state.modules.iter().filter(|m| {
                    m.floor == floor && m.kind == crate::model::ModuleKind::FireSuppressionTower
                }) {
                    if (tm.visual_x - d.x).hypot(tm.visual_z - d.z)
                        <= crate::disasters::SUPPRESSION_RADIUS
                    {
                        let (p1, _) = cam.project(
                            Vec3::new(tm.visual_x, fy + 1.38, tm.visual_z),
                            rect,
                            focus_y,
                        );
                        let (p2, _) =
                            cam.project(Vec3::new(d.x, fy + 1.25, d.z), rect, focus_y);
                        entity_overlays.push((
                            p1,
                            p2,
                            Stroke::new(3.0_f32, Color32::from_rgb(186, 230, 253)),
                        ));
                    }
                }
            }
        }

        for m in state.modules.iter().filter(|m| m.floor == floor) {
            build_module_parts(&mut parts, m, fy, time_s);
            if m.being_relocated {
                let (bp, _) = cam.project(
                    Vec3::new(m.visual_x, fy + 2.15, m.visual_z),
                    rect,
                    focus_y,
                );
                badges.push((bp, "OPTIMIZING".to_string(), Color32::from_rgb(255, 215, 75)));
            }
        }

        let floor_bots: Vec<&Robot> = state.robots.iter().filter(|r| r.floor == floor).collect();
        if state.has_tech("swarm_bft_mesh") {
            for i in 0..floor_bots.len() {
                for j in (i + 1)..floor_bots.len() {
                    let a = floor_bots[i];
                    let b = floor_bots[j];
                    if (a.x - b.x).powi(2) + (a.z - b.z).powi(2) < 32.0 {
                        let (pa, _) = cam.project(Vec3::new(a.x, fy + 0.95, a.z), rect, focus_y);
                        let (pb, _) = cam.project(Vec3::new(b.x, fy + 0.95, b.z), rect, focus_y);
                        entity_overlays.push((
                            pa,
                            pb,
                            Stroke::new(
                                1.2_f32,
                                Color32::from_rgba_unmultiplied(120, 220, 255, 95),
                            ),
                        ));
                    }
                }
            }
        }

        for r in floor_bots {
            build_robot_parts(&mut parts, r, fy, time_s);
            if let RobotTask::RelocatingModule {
                dest_gx, dest_gz, ..
            } = r.task
            {
                let (p1, _) = cam.project(Vec3::new(r.x, fy + 0.88, r.z), rect, focus_y);
                let (p2, _) = cam.project(
                    Vec3::new(
                        dest_gx as f32 * CELL_PITCH,
                        fy + 0.18,
                        dest_gz as f32 * CELL_PITCH,
                    ),
                    rect,
                    focus_y,
                );
                entity_overlays.push((p1, p2, Stroke::new(2.2_f32, Color32::from_rgb(255, 195, 55))));
            }
        }

        emit_parts_mesh(painter, cam, &view, sun_dir, rect, focus_y, &parts);

        for (p1, p2, stroke) in entity_overlays {
            painter.line_segment([p1, p2], stroke);
        }

        for (pos, text, col) in badges {
            if rect.contains(pos) {
                painter.text(
                    pos,
                    Align2::LEFT_BOTTOM,
                    text,
                    FontId::proportional(11.5),
                    col,
                );
            }
        }
    }
}

fn emit_oriented_box(
    quads: &mut Vec<ProjectedQuad>,
    cam: &IsoCamera,
    view: &Mat4,
    sun_dir: Vec3,
    rect: Rect,
    focus_y: f32,
    part: &BoxPart,
) {
    let h = part.size * 0.5;
    let local_corners = [
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(h.x, h.y, -h.z),
        Vec3::new(-h.x, h.y, -h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(h.x, h.y, h.z),
        Vec3::new(-h.x, h.y, h.z),
    ];
    let mut world_corners = [Vec3::ZERO; 8];
    for i in 0..8 {
        world_corners[i] = part.center + part.rot * local_corners[i];
    }

    let faces: [([usize; 4], Vec3); 6] = [
        ([3, 2, 6, 7], Vec3::Y),
        ([0, 4, 5, 1], -Vec3::Y),
        ([4, 5, 6, 7], Vec3::Z),
        ([1, 0, 3, 2], -Vec3::Z),
        ([5, 1, 2, 6], Vec3::X),
        ([0, 4, 7, 3], -Vec3::X),
    ];

    for (idx, local_n) in faces {
        let world_n = if part.rot == Quat::IDENTITY {
            local_n
        } else {
            (part.rot * local_n).normalize()
        };
        let vn = view.transform_vector3(world_n);
        if vn.z <= 0.0001 {
            continue;
        }
        let mut pts = [Pos2::ZERO; 4];
        let mut z_sum = 0.0f32;
        for (k, &ci) in idx.iter().enumerate() {
            let (sp, vz) = cam.project(world_corners[ci], rect, focus_y);
            pts[k] = sp;
            z_sum += vz;
        }
        let ndotl = world_n.dot(sun_dir).max(0.0);
        let shade = 0.48 + 0.14 * world_n.y + 0.56 * ndotl;
        let r = ((part.rgb[0] as f32) * shade).clamp(0.0, 255.0) as u8;
        let g = ((part.rgb[1] as f32) * shade).clamp(0.0, 255.0) as u8;
        let b = ((part.rgb[2] as f32) * shade).clamp(0.0, 255.0) as u8;
        quads.push(ProjectedQuad {
            pts,
            depth: z_sum * 0.25,
            color: Color32::from_rgb(r, g, b),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GameState;
    use egui::{LayerId, Order};

    #[test]
    fn floor_passes_render_slab_then_grid_then_entities_without_back_occlusion() {
        let mut state = GameState::new_starter_factory();
        state.main_floor_half_extent = 13; // 26x26 megafactory floor
        state.warehouse_flare_tier = 5;

        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 1000.0));
        let mut cam = IsoCamera::default();
        cam.view_mode = FloorViewMode::Single(FloorId::MainFloor);

        let full = ctx.run(egui::RawInput::default(), |c| {
            let painter = Painter::new(c.clone(), LayerId::new(Order::Middle, egui::Id::new("t")), rect);
            render_isometric_factory(&painter, rect, &state, &cam);
        });

        // Shape 0: background rect_filled
        // Shape 1: Pass A floor slab Shape::Mesh (exactly 1 box -> 3 visible faces = 12 vertices)
        // Shapes 2..K: Pass B grid line segments
        // Shape K: Pass C entities Shape::Mesh (bollards, shaft, modules, robots, drones, busbar)
        let shapes = &full.shapes;
        assert!(shapes.len() > 40, "Expected background + slab + grid lines + entity mesh");
        match &shapes[1].shape {
            Shape::Mesh(slab_mesh) => {
                assert_eq!(
                    slab_mesh.vertices.len(),
                    12,
                    "Pass A slab mesh must contain ONLY the 3 visible faces of the floor slab"
                );
            }
            other => panic!("Expected Pass A slab Shape::Mesh at shapes[1], got {:?}", other),
        }
        let last_mesh_idx = shapes
            .iter()
            .rposition(|cs| matches!(cs.shape, Shape::Mesh(_)))
            .expect("Expected Pass C entity mesh");
        assert!(
            last_mesh_idx > 2,
            "Pass C entity mesh must follow Pass B grid lines"
        );
        if let Shape::Mesh(ent_mesh) = &shapes[last_mesh_idx].shape {
            assert!(
                ent_mesh.vertices.len() > 200,
                "Expected full 3D entities (including back-half shaft & bollards) in Pass C"
            );
        }
    }

    #[test]
    fn long_pipes_and_busbar_subdivide_into_short_segments() {
        let mut parts = Vec::new();
        push_subdivided_axis_box(
            &mut parts,
            Vec3::new(0.0, 0.14, 0.0),
            Vec3::new(26.0 * CELL_PITCH * 0.8, 0.20, 0.22),
            [217, 119, 6],
            2.0,
        );
        assert!(parts.len() >= 25);
        for p in &parts {
            assert!(p.size.x <= 2.01 && p.size.z <= 2.01);
        }
    }
}
