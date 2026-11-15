#![forbid(unsafe_code)]
//! Detailed procedural 3D CAD sub-mesh primitives for all 7 Robot classes
//! and all 19 Factory / Basement / Lab / Admin modules.

use crate::model::{FactoryModule, ModuleKind, Robot, RobotKind, RobotTask};
use glam::{Quat, Vec3};

/// A single oriented 3D box primitive in world space.
#[derive(Debug, Clone, Copy)]
pub struct BoxPart {
    pub center: Vec3,
    pub size: Vec3,
    pub rot: Quat,
    pub rgb: [u8; 3],
}

impl BoxPart {
    pub fn axis_aligned(center: Vec3, size: Vec3, rgb: [u8; 3]) -> Self {
        Self {
            center,
            size,
            rot: Quat::IDENTITY,
            rgb,
        }
    }

    pub fn oriented(center: Vec3, size: Vec3, rot: Quat, rgb: [u8; 3]) -> Self {
        Self {
            center,
            size,
            rot,
            rgb,
        }
    }
}

/// Helper to place a local part on an entity centered at `origin` rotated by `yaw`.
fn local_part(
    origin: Vec3,
    yaw: f32,
    local_offset: Vec3,
    size: Vec3,
    local_rot: Quat,
    rgb: [u8; 3],
) -> BoxPart {
    let q_yaw = Quat::from_rotation_y(-yaw);
    BoxPart {
        center: origin + q_yaw * local_offset,
        size,
        rot: q_yaw * local_rot,
        rgb,
    }
}

/// Builds unique, multi-part articulated 3D geometry for each of the 7 Robot classes.
pub fn build_robot_parts(out: &mut Vec<BoxPart>, r: &Robot, fy: f32, time_s: f32) {
    let phase = time_s * 5.5 + (r.id as f32) * 1.37;
    let working = matches!(r.task, RobotTask::Operating { .. });
    let carrying = matches!(r.task, RobotTask::CarryingPart { .. });
    let relocating = matches!(
        r.task,
        RobotTask::MovingToRelocate { .. } | RobotTask::RelocatingModule { .. }
    );
    let swing = if working { (phase * 1.8).sin() * 0.42 } else { 0.12 };
    let origin = Vec3::new(r.x, fy, r.z);
    let yaw = r.heading;

    match r.kind {
        RobotKind::BasicWorker => {
            // Mk-I Wheeled Utility Bot: dual rubber tires, hazard-yellow hull, rear battery LEDs, single pincer arm, binocular stalk & amber beacon
            let dark_tire = [30, 35, 44];
            let hub = [170, 180, 195];
            let yellow = [242, 186, 48];
            let stripe = [42, 46, 54];
            for side in [-0.30f32, 0.30] {
                out.push(local_part(origin, yaw, Vec3::new(0.04, 0.14, side), Vec3::new(0.32, 0.28, 0.10), Quat::IDENTITY, dark_tire));
                out.push(local_part(origin, yaw, Vec3::new(0.04, 0.14, side * 1.18), Vec3::new(0.14, 0.14, 0.04), Quat::IDENTITY, hub));
            }
            // Rear caster
            out.push(local_part(origin, yaw, Vec3::new(-0.24, 0.09, 0.0), Vec3::new(0.14, 0.16, 0.10), Quat::IDENTITY, dark_tire));
            // Chamfered main body + hazard bumper
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.36, 0.0), Vec3::new(0.50, 0.34, 0.46), Quat::IDENTITY, yellow));
            out.push(local_part(origin, yaw, Vec3::new(0.24, 0.22, 0.0), Vec3::new(0.08, 0.10, 0.48), Quat::IDENTITY, stripe));
            // Rear battery pack + green charge bar
            out.push(local_part(origin, yaw, Vec3::new(-0.27, 0.38, 0.0), Vec3::new(0.12, 0.24, 0.30), Quat::IDENTITY, [55, 65, 80]));
            out.push(local_part(origin, yaw, Vec3::new(-0.34, 0.40, 0.0), Vec3::new(0.03, 0.16, 0.18), Quat::IDENTITY, [60, 235, 130]));
            // Articulated utility arm on right front
            let q_arm = Quat::from_rotation_z(-0.25 + swing);
            out.push(local_part(origin, yaw, Vec3::new(0.26, 0.40, 0.22), Vec3::new(0.34, 0.08, 0.08), q_arm, [120, 135, 155]));
            out.push(local_part(origin, yaw, Vec3::new(0.42, 0.38 + swing * 0.12, 0.22), Vec3::new(0.12, 0.12, 0.14), Quat::IDENTITY, [250, 210, 90]));
            // Binocular camera head + amber strobe dome
            out.push(local_part(origin, yaw, Vec3::new(0.04, 0.64, 0.0), Vec3::new(0.28, 0.20, 0.32), Quat::IDENTITY, [226, 232, 240]));
            for eye_z in [-0.09f32, 0.09] {
                out.push(local_part(origin, yaw, Vec3::new(0.19, 0.65, eye_z), Vec3::new(0.06, 0.09, 0.09), Quat::IDENTITY, [35, 225, 245]));
            }
            out.push(local_part(origin, yaw, Vec3::new(-0.02, 0.79, 0.0), Vec3::new(0.10, 0.10, 0.10), Quat::IDENTITY, [255, 165, 35]));
        }

        RobotKind::Assembler => {
            // 6-Axis Dual-Arm SCARA Manipulator Bot: 4 Mecanum rollers, rotating cobalt turret, twin articulated arms + cyan weld tip
            for (ox, oz) in [(-0.24, -0.24), (0.24, -0.24), (-0.24, 0.24), (0.24, 0.24)] {
                out.push(local_part(origin, yaw, Vec3::new(ox, 0.10, oz), Vec3::new(0.18, 0.18, 0.12), Quat::IDENTITY, [35, 42, 55]));
            }
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.20, 0.0), Vec3::new(0.58, 0.12, 0.58), Quat::IDENTITY, [52, 65, 84]));
            let turret_yaw = yaw + if working { (phase * 1.1).sin() * 0.35 } else { 0.0 };
            out.push(local_part(origin, turret_yaw, Vec3::new(0.0, 0.46, 0.0), Vec3::new(0.42, 0.40, 0.42), Quat::IDENTITY, [56, 152, 242]));
            // Dual shoulder pods + articulated upper/forearms
            for (side, phase_off) in [(-0.28f32, 0.0f32), (0.28, 1.6)] {
                let a = if working { (phase * 2.0 + phase_off).sin() * 0.48 } else { 0.20 };
                out.push(local_part(origin, turret_yaw, Vec3::new(0.05, 0.54, side), Vec3::new(0.14, 0.14, 0.14), Quat::IDENTITY, [220, 230, 245]));
                out.push(local_part(origin, turret_yaw, Vec3::new(0.24, 0.52, side), Vec3::new(0.32, 0.09, 0.09), Quat::from_rotation_z(a * 0.6), [148, 163, 184]));
                let tip_col = if side > 0.0 && working { [55, 245, 255] } else { [250, 195, 65] };
                out.push(local_part(origin, turret_yaw, Vec3::new(0.42, 0.48 + a * 0.12, side * 0.85), Vec3::new(0.12, 0.10, 0.12), Quat::IDENTITY, tip_col));
            }
            // Cyclops laser scanner head
            out.push(local_part(origin, turret_yaw, Vec3::new(0.04, 0.76, 0.0), Vec3::new(0.28, 0.18, 0.30), Quat::IDENTITY, [30, 41, 59]));
            out.push(local_part(origin, turret_yaw, Vec3::new(0.19, 0.76, 0.0), Vec3::new(0.04, 0.08, 0.24), Quat::IDENTITY, [56, 235, 255]));
        }

        RobotKind::Hauler => {
            // 6-Wheel Heavy Flatbed AGV Forklift: long emerald deck, front vertical twin-mast hydraulic forks, guide rails & rear strobes
            for ox in [-0.30f32, 0.0, 0.30] {
                for oz in [-0.28f32, 0.28] {
                    out.push(local_part(origin, yaw, Vec3::new(ox, 0.09, oz), Vec3::new(0.16, 0.18, 0.09), Quat::IDENTITY, [28, 34, 44]));
                }
            }
            // Low-slung AGV deck + safety bumpers
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.22, 0.0), Vec3::new(0.84, 0.14, 0.56), Quat::IDENTITY, [42, 205, 142]));
            out.push(local_part(origin, yaw, Vec3::new(0.44, 0.20, 0.0), Vec3::new(0.06, 0.10, 0.58), Quat::IDENTITY, [245, 190, 50]));
            // Twin hydraulic mast uprights at front + sliding fork tines
            for oz in [-0.16f32, 0.16] {
                out.push(local_part(origin, yaw, Vec3::new(0.30, 0.50, oz), Vec3::new(0.07, 0.52, 0.07), Quat::IDENTITY, [71, 85, 105]));
            }
            let fork_y = if carrying { 0.56 } else { 0.28 };
            for oz in [-0.14f32, 0.14] {
                out.push(local_part(origin, yaw, Vec3::new(0.46, fork_y, oz), Vec3::new(0.28, 0.05, 0.07), Quat::IDENTITY, [203, 213, 225]));
            }
            // Rear sensor cab & strobe stalks
            out.push(local_part(origin, yaw, Vec3::new(-0.28, 0.40, 0.0), Vec3::new(0.24, 0.22, 0.42), Quat::IDENTITY, [30, 41, 59]));
            out.push(local_part(origin, yaw, Vec3::new(-0.34, 0.58, -0.16), Vec3::new(0.06, 0.14, 0.06), Quat::IDENTITY, [255, 185, 45]));
            out.push(local_part(origin, yaw, Vec3::new(-0.34, 0.58, 0.16), Vec3::new(0.06, 0.14, 0.06), Quat::IDENTITY, [56, 230, 245]));
        }

        RobotKind::Builder => {
            // Spider-Legged Heavy Structural Architect Mech: 4 articulated hydraulic legs, roll-cage orange cab, dorsal crane boom & gyro core
            let gait = (phase * 1.4).sin() * 0.06;
            for (i, (lx, lz)) in [(-0.28f32, -0.28f32), (0.28, -0.28), (-0.28, 0.28), (0.28, 0.28)].iter().enumerate() {
                let step_h = if i % 2 == 0 { gait.max(0.0) } else { (-gait).max(0.0) };
                // Upper outrigger thigh + vertical shin + footpad
                out.push(local_part(origin, yaw, Vec3::new(*lx * 1.15, 0.28 + step_h, *lz * 1.15), Vec3::new(0.22, 0.09, 0.10), Quat::from_rotation_y(lz.atan2(*lx)), [71, 85, 105]));
                out.push(local_part(origin, yaw, Vec3::new(*lx * 1.45, 0.14 + step_h, *lz * 1.45), Vec3::new(0.10, 0.26, 0.10), Quat::IDENTITY, [249, 115, 22]));
                out.push(local_part(origin, yaw, Vec3::new(*lx * 1.45, 0.03 + step_h, *lz * 1.45), Vec3::new(0.16, 0.06, 0.16), Quat::IDENTITY, [45, 55, 72]));
            }
            // Heavy armored cab + roll-cage frame
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.44, 0.0), Vec3::new(0.54, 0.34, 0.52), Quat::IDENTITY, [249, 115, 22]));
            out.push(local_part(origin, yaw, Vec3::new(0.28, 0.46, 0.0), Vec3::new(0.05, 0.18, 0.38), Quat::IDENTITY, [253, 224, 71]));
            // Dorsal articulated crane boom + plasma projector forks
            let boom_pitch = if relocating { -0.45 } else { -0.18 };
            out.push(local_part(origin, yaw, Vec3::new(-0.04, 0.76, 0.0), Vec3::new(0.52, 0.11, 0.14), Quat::from_rotation_z(boom_pitch), [234, 179, 8]));
            out.push(local_part(origin, yaw, Vec3::new(0.24, 0.88, -0.12), Vec3::new(0.14, 0.10, 0.08), Quat::IDENTITY, [56, 235, 255]));
            out.push(local_part(origin, yaw, Vec3::new(0.24, 0.88, 0.12), Vec3::new(0.14, 0.10, 0.08), Quat::IDENTITY, [56, 235, 255]));
        }

        RobotKind::Maintenance => {
            // Caterpillar-Tracked Basement Utility & Pipe-Welder Rover: dual tank treads, rear pressurized coolant tank, front wrench & scanner dish
            for oz in [-0.26f32, 0.26] {
                out.push(local_part(origin, yaw, Vec3::new(0.0, 0.13, oz), Vec3::new(0.66, 0.22, 0.14), Quat::IDENTITY, [38, 44, 56]));
                out.push(local_part(origin, yaw, Vec3::new(0.0, 0.13, oz * 1.15), Vec3::new(0.46, 0.10, 0.04), Quat::IDENTITY, [120, 132, 150]));
            }
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.34, 0.0), Vec3::new(0.52, 0.28, 0.42), Quat::IDENTITY, [225, 78, 92]));
            // Dorsal cyan pressurized coolant cylinder
            out.push(local_part(origin, yaw, Vec3::new(-0.16, 0.56, 0.0), Vec3::new(0.24, 0.20, 0.38), Quat::IDENTITY, [34, 211, 238]));
            // Front wrench arm + sweeping ultrasonic scanner dish
            out.push(local_part(origin, yaw, Vec3::new(0.32, 0.36, 0.14), Vec3::new(0.26, 0.08, 0.10), Quat::from_rotation_z(swing * 0.5), [203, 213, 225]));
            let dish_yaw = yaw + (phase * 0.9).sin() * 0.5;
            out.push(local_part(origin, dish_yaw, Vec3::new(0.10, 0.62, -0.10), Vec3::new(0.08, 0.20, 0.22), Quat::IDENTITY, [251, 191, 36]));
        }

        RobotKind::Researcher => {
            // Hovering Clean-Room Lab Droid: glowing cyan air-bearing hover ring, ceramic white cylinder, chest sample carousel & tri-optic microscope head
            let hover = 0.08 + (phase * 0.8).sin() * 0.04;
            let h_org = origin + Vec3::new(0.0, hover, 0.0);
            out.push(local_part(h_org, yaw, Vec3::new(0.0, 0.06, 0.0), Vec3::new(0.52, 0.08, 0.52), Quat::IDENTITY, [34, 211, 238]));
            out.push(local_part(h_org, yaw, Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.44, 0.10, 0.44), Quat::IDENTITY, [30, 41, 59]));
            out.push(local_part(h_org, yaw, Vec3::new(0.0, 0.42, 0.0), Vec3::new(0.38, 0.46, 0.38), Quat::IDENTITY, [241, 245, 249]));
            // Glowing cyan sample-vial chest bay
            out.push(local_part(h_org, yaw, Vec3::new(0.19, 0.44, 0.0), Vec3::new(0.05, 0.18, 0.22), Quat::IDENTITY, [56, 235, 255]));
            // Delicate dual pipette arms
            for oz in [-0.24f32, 0.24] {
                out.push(local_part(h_org, yaw, Vec3::new(0.16, 0.44, oz), Vec3::new(0.24, 0.05, 0.05), Quat::from_rotation_z(swing * 0.4), [148, 163, 184]));
            }
            // Tri-optic spectrometer head
            out.push(local_part(h_org, yaw, Vec3::new(0.0, 0.76, 0.0), Vec3::new(0.28, 0.20, 0.28), Quat::IDENTITY, [226, 232, 240]));
            out.push(local_part(h_org, yaw, Vec3::new(0.15, 0.78, 0.0), Vec3::new(0.06, 0.07, 0.18), Quat::IDENTITY, [14, 165, 233]));
        }

        RobotKind::Administrator => {
            // Brushed-Titanium & Gold-Trim Executive Droid (Floor 3): spherical monowheel pod, tall sculpted violet-titanium torso, gold pauldrons & floating holo-tablet
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.28, 0.28, 0.28), Quat::IDENTITY, [30, 41, 59]));
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.52, 0.0), Vec3::new(0.38, 0.52, 0.34), Quat::IDENTITY, [139, 92, 246]));
            // Gold executive shoulder pauldrons & illuminated tie strip
            for oz in [-0.23f32, 0.23] {
                out.push(local_part(origin, yaw, Vec3::new(0.0, 0.72, oz), Vec3::new(0.26, 0.10, 0.14), Quat::IDENTITY, [250, 204, 21]));
            }
            out.push(local_part(origin, yaw, Vec3::new(0.20, 0.54, 0.0), Vec3::new(0.03, 0.32, 0.07), Quat::IDENTITY, [250, 204, 21]));
            // Crown visor head
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.90, 0.0), Vec3::new(0.26, 0.22, 0.26), Quat::IDENTITY, [226, 232, 240]));
            out.push(local_part(origin, yaw, Vec3::new(0.14, 0.91, 0.0), Vec3::new(0.04, 0.08, 0.22), Quat::IDENTITY, [250, 204, 21]));
            // Floating cyan holographic chart tablet beside right hand
            let tab_bob = (phase * 0.9).sin() * 0.05;
            out.push(local_part(origin, yaw, Vec3::new(0.32, 0.64 + tab_bob, 0.24), Vec3::new(0.18, 0.14, 0.03), Quat::from_rotation_y(-0.4), [56, 235, 255]));
        }

        RobotKind::RepairBot => {
            // Emergency Field Repair & Fire-Response Bot: lime/white tracked responder, twin red fire canisters, blue/red strobe bar & foam/welder nozzle
            for oz in [-0.25f32, 0.25] {
                out.push(local_part(origin, yaw, Vec3::new(0.0, 0.12, oz), Vec3::new(0.62, 0.20, 0.12), Quat::IDENTITY, [30, 41, 59]));
            }
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.34, 0.0), Vec3::new(0.50, 0.28, 0.42), Quat::IDENTITY, [132, 204, 22]));
            // Twin flank red Halon/Cryo tanks
            for oz in [-0.24f32, 0.24] {
                out.push(local_part(origin, yaw, Vec3::new(-0.14, 0.48, oz), Vec3::new(0.24, 0.14, 0.12), Quat::IDENTITY, [239, 68, 68]));
            }
            // Front cryo-foam / welder boom
            out.push(local_part(origin, yaw, Vec3::new(0.30, 0.40, 0.0), Vec3::new(0.28, 0.08, 0.08), Quat::from_rotation_z(swing * 0.5), [226, 232, 240]));
            out.push(local_part(origin, yaw, Vec3::new(0.46, 0.42, 0.0), Vec3::new(0.10, 0.10, 0.10), Quat::IDENTITY, [56, 235, 255]));
            // Emergency flashing blue/red lightbar
            let flash_blue = ((time_s * 10.0) as i32) % 2 == 0;
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.58, -0.10), Vec3::new(0.12, 0.09, 0.14), Quat::IDENTITY, if flash_blue { [59, 130, 246] } else { [239, 68, 68] }));
            out.push(local_part(origin, yaw, Vec3::new(0.0, 0.58, 0.10), Vec3::new(0.12, 0.09, 0.14), Quat::IDENTITY, if flash_blue { [239, 68, 68] } else { [59, 130, 246] }));
        }

        RobotKind::MotivationDrone => {
            // Overhead CheerBot Aerial Drone (y = 2.6u): coaxial ducted fans, cheerful gold/magenta chassis, LED smiley visor & twin megaphone cones!
            let fly_org = origin + Vec3::new(0.0, 2.55 + (phase * 0.9).sin() * 0.14, 0.0);
            out.push(local_part(fly_org, yaw, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.44, 0.22, 0.44), Quat::IDENTITY, [250, 204, 21]));
            out.push(local_part(fly_org, time_s * 14.0, Vec3::new(0.0, 0.16, 0.0), Vec3::new(0.62, 0.03, 0.09), Quat::IDENTITY, [244, 114, 182]));
            out.push(local_part(fly_org, -time_s * 14.0, Vec3::new(0.0, 0.20, 0.0), Vec3::new(0.09, 0.03, 0.62), Quat::IDENTITY, [56, 235, 255]));
            // Megaphone side speakers + smiley LED faceplate
            for oz in [-0.28f32, 0.28] {
                out.push(local_part(fly_org, yaw, Vec3::new(0.08, -0.02, oz), Vec3::new(0.18, 0.12, 0.14), Quat::IDENTITY, [236, 72, 153]));
            }
            out.push(local_part(fly_org, yaw, Vec3::new(0.23, 0.0, 0.0), Vec3::new(0.04, 0.12, 0.26), Quat::IDENTITY, [74, 222, 128]));
        }
    }

    // Carried part crate rendered directly on forks (Hauler) or above chassis
    if let RobotTask::CarryingPart { product_tier, .. } = r.task {
        let crate_rgb = match product_tier {
            1 => [250, 204, 21],
            2 => [56, 189, 248],
            3 => [168, 85, 247],
            4 => [52, 211, 153],
            _ => [244, 114, 182],
        };
        let offset = if r.kind == RobotKind::Hauler {
            Vec3::new(0.46, 0.74, 0.0)
        } else {
            Vec3::new(0.28, 0.72, 0.0)
        };
        out.push(local_part(origin, yaw, offset, Vec3::new(0.30, 0.26, 0.30), Quat::IDENTITY, crate_rgb));
        out.push(local_part(origin, yaw, offset + Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.32, 0.04, 0.10), Quat::IDENTITY, [30, 41, 59]));
    }
}

/// Builds unique, detailed 3D architectural & mechanical sub-parts for each of the 19 Module types.
pub fn build_module_parts(out: &mut Vec<BoxPart>, m: &FactoryModule, fy: f32, time_s: f32) {
    if m.kind == ModuleKind::VerticalFreightLift {
        return; // Drawn by dedicated shaft truss + moving elevator car
    }
    let lift_y = if m.being_relocated {
        0.52 + (time_s * 7.0).sin() * 0.06
    } else {
        0.0
    };
    let c = Vec3::new(m.visual_x, fy + lift_y, m.visual_z);
    let active = m.work_progress > 0.01 || m.output_buffer > 0;
    let spin = time_s * if active { 4.2 } else { 0.9 };

    // Shared chamfered anchor plinth
    out.push(BoxPart::axis_aligned(
        c + Vec3::new(0.0, 0.08, 0.0),
        Vec3::new(1.62, 0.16, 1.62),
        [30, 41, 59],
    ));

    match m.kind {
        ModuleKind::ProtoBench => {
            // 4-leg open steel workbench, butcher-block/amber top, pegboard tool backplane, vise & swing-arm lamp
            for (lx, lz) in [(-0.62, -0.45), (0.62, -0.45), (-0.62, 0.45), (0.62, 0.45)] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(lx, 0.44, lz), Vec3::new(0.10, 0.56, 0.10), [100, 116, 139]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.74, 0.0), Vec3::new(1.42, 0.10, 1.06), [217, 155, 56]));
            // Pegboard backplane + tool bars
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.12, -0.48), Vec3::new(1.36, 0.68, 0.08), [51, 65, 85]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(-0.35, 1.15, -0.42), Vec3::new(0.42, 0.12, 0.06), [56, 189, 248]));
            // Benchtop vise + reciprocating mini-mill head
            let slide = if active { (spin * 1.4).sin() * 0.25 } else { 0.0 };
            out.push(BoxPart::axis_aligned(c + Vec3::new(slide, 0.90, 0.08), Vec3::new(0.28, 0.22, 0.28), [250, 204, 21]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.48, 1.38, -0.25), Vec3::new(0.32, 0.08, 0.36), [253, 224, 71]));
        }

        ModuleKind::RawIntakeHopper => {
            // 4 corner legs + wide stepped inverted funnel hopper + raw billet pile + side vibratory chute
            for (lx, lz) in [(-0.56, -0.56), (0.56, -0.56), (-0.56, 0.56), (0.56, 0.56)] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(lx, 0.48, lz), Vec3::new(0.12, 0.68, 0.12), [71, 85, 105]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.62, 0.0), Vec3::new(0.76, 0.34, 0.76), [100, 116, 139]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.98, 0.0), Vec3::new(1.32, 0.42, 1.32), [148, 163, 184]));
            // Glowing raw alloy stock inside hopper
            if m.input_buffer > 0 || m.output_buffer > 0 {
                out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.22, 0.0), Vec3::new(0.96, 0.16, 0.96), [217, 119, 6]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.58, 0.36, 0.0), Vec3::new(0.46, 0.16, 0.48), [234, 179, 8]));
        }

        ModuleKind::FabricationCell => {
            // CNC 5-Axis Machining Center: cobalt enclosure, dark viewport bay, sliding X-Z overhead spindle & side chip auger
            out.push(BoxPart::axis_aligned(c + Vec3::new(-0.08, 0.72, -0.36), Vec3::new(1.38, 1.12, 0.56), [37, 99, 235]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(-0.08, 0.42, 0.24), Vec3::new(1.38, 0.52, 0.66), [59, 130, 246]));
            //Overhead CNC bridge + oscillating milling spindle
            out.push(BoxPart::axis_aligned(c + Vec3::new(-0.08, 1.22, 0.12), Vec3::new(1.34, 0.16, 0.88), [226, 232, 240]));
            let sx = if active { (spin * 1.5).sin() * 0.34 } else { 0.0 };
            out.push(BoxPart::oriented(
                c + Vec3::new(sx, 0.92, 0.20),
                Vec3::new(0.20, 0.44, 0.20),
                Quat::from_rotation_y(spin),
                [56, 235, 255],
            ));
            // Side chip-auger chute
            out.push(BoxPart::oriented(
                c + Vec3::new(0.70, 0.48, 0.0),
                Vec3::new(0.26, 0.58, 0.28),
                Quat::from_rotation_z(-0.35),
                [71, 85, 105],
            ));
        }

        ModuleKind::SubAssemblyCell => {
            // High-Speed SMT Pick-and-Place: emerald PCB rail bed, side component tape-reel banks & fast X-Y gantry head
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.44, 0.0), Vec3::new(1.44, 0.56, 0.92), [13, 148, 136]));
            // PCB substrate track
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.75, 0.0), Vec3::new(1.48, 0.06, 0.44), [22, 163, 74]));
            // Component tape feeder reels along front & back
            for rx in [-0.45f32, -0.15, 0.15, 0.45] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(rx, 0.72, 0.54), Vec3::new(0.18, 0.28, 0.10), [226, 232, 240]));
                out.push(BoxPart::axis_aligned(c + Vec3::new(rx, 0.72, -0.54), Vec3::new(0.18, 0.28, 0.10), [226, 232, 240]));
            }
            // Rapid X-Y pick-and-place gantry bar
            let gx = if active { (spin * 2.4).sin() * 0.42 } else { 0.0 };
            out.push(BoxPart::axis_aligned(c + Vec3::new(gx, 1.02, 0.0), Vec3::new(0.16, 0.14, 1.04), [248, 250, 252]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(gx, 0.92, (spin * 3.1).cos() * 0.18), Vec3::new(0.18, 0.22, 0.18), [250, 204, 21]));
        }

        ModuleKind::FinalAssemblyCell => {
            // Dual-Arm Robotic Weld & Integration Cell: central rotating jig + TWO articulated KUKA-style robot arms
            out.push(BoxPart::oriented(
                c + Vec3::new(0.0, 0.34, 0.0),
                Vec3::new(0.68, 0.36, 0.68),
                Quat::from_rotation_y(spin * 0.4),
                [71, 85, 105],
            ));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.62, 0.0), Vec3::new(0.42, 0.24, 0.42), [56, 189, 248]));
            // Twin corner articulated welding arms
            for (side, p_off) in [(-0.54f32, 0.0f32), (0.54, 2.1)] {
                let bow = if active { (spin * 1.6 + p_off).sin() * 0.38 } else { 0.15 };
                out.push(BoxPart::axis_aligned(c + Vec3::new(side, 0.38, -0.36), Vec3::new(0.24, 0.44, 0.24), [139, 92, 246]));
                out.push(BoxPart::oriented(
                    c + Vec3::new(side * 0.62, 0.84, -0.16),
                    Vec3::new(0.52, 0.12, 0.12),
                    Quat::from_rotation_z(if side < 0.0 { -0.45 + bow } else { 0.45 - bow }),
                    [249, 115, 22],
                ));
                out.push(BoxPart::axis_aligned(c + Vec3::new(side * 0.28, 0.88, 0.0), Vec3::new(0.12, 0.12, 0.12), [253, 224, 71]));
            }
        }

        ModuleKind::QualityPackagingBay => {
            // Laser Metrology Arch & Strapping Tunnel: roller conveyor + twin emerald scanner arches + sweeping laser bar
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.34, 0.0), Vec3::new(1.52, 0.36, 0.72), [51, 65, 85]));
            for ax in [-0.36f32, 0.36] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(ax, 0.78, -0.44), Vec3::new(0.14, 0.92, 0.14), [34, 197, 94]));
                out.push(BoxPart::axis_aligned(c + Vec3::new(ax, 0.78, 0.44), Vec3::new(0.14, 0.92, 0.14), [34, 197, 94]));
                out.push(BoxPart::axis_aligned(c + Vec3::new(ax, 1.22, 0.0), Vec3::new(0.16, 0.12, 1.02), [34, 197, 94]));
            }
            let scan_y = 0.65 + (time_s * 3.5).sin().abs() * 0.42;
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, scan_y, 0.0), Vec3::new(0.06, 0.05, 0.84), [134, 239, 172]));
        }

        ModuleKind::DeliveryDock => {
            // Autonomous Logistics Bay: hazard loading ramp, twin dock portals, radar dispatch mast & cargo drone pod
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.24, 0.0), Vec3::new(1.66, 0.20, 1.66), [71, 85, 105]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.72, -0.58), Vec3::new(1.54, 0.82, 0.34), [234, 88, 12]));
            // Radar mast
            out.push(BoxPart::axis_aligned(c + Vec3::new(-0.62, 1.35, -0.58), Vec3::new(0.10, 0.56, 0.10), [148, 163, 184]));
            out.push(BoxPart::oriented(
                c + Vec3::new(-0.62, 1.66, -0.58),
                Vec3::new(0.38, 0.10, 0.08),
                Quat::from_rotation_y(time_s * 2.2),
                [56, 235, 255],
            ));
            // Outbound cargo drone: flies dramatic climb -> skyward arc -> return landing sortie!
            let p = m.drone_sortie_progress;
            let drone_offset = if p <= 0.001 {
                Vec3::new(0.16, 0.46 + (time_s * 3.0).sin().abs() * 0.12, 0.24)
            } else if p < 0.25 {
                let u = p / 0.25;
                Vec3::new(0.16, 0.46 + u * 5.2, 0.24)
            } else if p < 0.65 {
                let u = (p - 0.25) / 0.40;
                Vec3::new(0.16 + u * 22.0, 5.66 + u * 4.5, 0.24 - u * 16.0)
            } else {
                let u = 1.0 - (p - 0.65) / 0.35;
                Vec3::new(0.16 + u * 14.0, 0.46 + u * 5.8, 0.24 + u * 10.0)
            };
            out.push(BoxPart::axis_aligned(c + drone_offset, Vec3::new(0.58, 0.20, 0.58), [250, 204, 21]));
            out.push(BoxPart::axis_aligned(c + drone_offset + Vec3::new(0.0, -0.14, 0.0), Vec3::new(0.32, 0.16, 0.32), [56, 189, 248]));
        }

        ModuleKind::BotRepairStation => {
            // Autonomous Robot Pit-Stop Service Bay: drive-on diagnostic pad, twin green calibration portals & overhead micro-welder arm
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.20, 0.0), Vec3::new(1.48, 0.08, 1.48), [20, 83, 45]));
            for sx in [-0.58f32, 0.58] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(sx, 0.74, 0.0), Vec3::new(0.14, 1.08, 0.96), [34, 197, 94]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.28, 0.0), Vec3::new(1.30, 0.12, 0.42), [226, 232, 240]));
            let pulse_y = 0.62 + (time_s * 3.8).sin() * 0.22;
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, pulse_y, 0.0), Vec3::new(0.98, 0.05, 0.98), [74, 222, 128]));
        }

        ModuleKind::FireSuppressionTower => {
            // Cryo-Foam & Halon Suppression Turret: twin pressurized crimson tanks, rotating white/cyan foam cannon & strobe dome
            for tx in [-0.34f32, 0.34] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(tx, 0.68, -0.18), Vec3::new(0.36, 1.04, 0.36), [220, 38, 38]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.88, 0.18), Vec3::new(0.46, 0.82, 0.46), [241, 245, 249]));
            out.push(BoxPart::oriented(
                c + Vec3::new(0.0, 1.38, 0.18),
                Vec3::new(0.62, 0.18, 0.22),
                Quat::from_rotation_y(time_s * 1.4),
                [56, 235, 255],
            ));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.58, 0.18), Vec3::new(0.18, 0.16, 0.18), [239, 68, 68]));
        }

        ModuleKind::StorageSilo => {
            // Twin tall cylindrical-style octagonal silos + overhead catwalk bridge + sight gauge
            for sx in [-0.38f32, 0.38] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(sx, 0.98, 0.0), Vec3::new(0.62, 1.68, 0.62), [100, 116, 139]));
                out.push(BoxPart::axis_aligned(c + Vec3::new(sx, 1.88, 0.0), Vec3::new(0.46, 0.16, 0.46), [148, 163, 184]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.72, 0.0), Vec3::new(1.18, 0.10, 0.26), [234, 179, 8]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(-0.38, 0.95, 0.33), Vec3::new(0.08, 1.15, 0.04), [56, 235, 255]));
        }

        ModuleKind::PowerSubstation => {
            // Ribbed HV transformer core + 3 porcelain insulator bushings + copper corona rings
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.62, 0.0), Vec3::new(1.24, 0.92, 0.96), [71, 85, 105]));
            for bx in [-0.40f32, 0.0, 0.40] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(bx, 1.32, 0.0), Vec3::new(0.16, 0.52, 0.16), [226, 232, 240]));
                out.push(BoxPart::axis_aligned(c + Vec3::new(bx, 1.60, 0.0), Vec3::new(0.24, 0.08, 0.24), [250, 204, 21]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.64, 0.0), Vec3::new(1.05, 0.05, 0.05), [56, 235, 255]));
        }

        ModuleKind::PlumbingChiller => {
            // Twin horizontal cryo vessels + dual spinning overhead exhaust fans
            for cz in [-0.34f32, 0.34] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.54, cz), Vec3::new(1.36, 0.52, 0.48), [14, 165, 233]));
            }
            for fx in [-0.34f32, 0.34] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(fx, 0.94, 0.0), Vec3::new(0.54, 0.24, 0.54), [30, 41, 59]));
                out.push(BoxPart::oriented(
                    c + Vec3::new(fx, 1.08, 0.0),
                    Vec3::new(0.44, 0.04, 0.10),
                    Quat::from_rotation_y(time_s * 6.0),
                    [186, 230, 253],
                ));
            }
        }

        ModuleKind::HydraulicPipingManifold => {
            // 4 vertical pressure accumulator bottles + emerald/copper crossover header pipes
            for (px, pz) in [(-0.38f32, -0.34f32), (0.38, -0.34), (-0.38, 0.34), (0.38, 0.34)] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(px, 0.68, pz), Vec3::new(0.28, 1.04, 0.28), [16, 185, 129]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.24, 0.0), Vec3::new(1.12, 0.12, 0.96), [217, 119, 6]));
        }

        ModuleKind::MaintenanceBay => {
            // Dual-post hydraulic service rack + red roll-around tool chest + calibration arch
            for px in [-0.54f32, 0.54] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(px, 0.76, 0.0), Vec3::new(0.14, 1.20, 0.18), [245, 158, 11]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(-0.46, 0.44, -0.48), Vec3::new(0.48, 0.54, 0.32), [225, 29, 72]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.32, 0.0), Vec3::new(1.22, 0.10, 0.16), [71, 85, 105]));
        }

        ModuleKind::SwarmSimMainframe => {
            // 4 tall obsidian/cyan liquid-cooled server monoliths + central levitating neural core
            for (mx, mz) in [(-0.44f32, -0.44f32), (0.44, -0.44), (-0.44, 0.44), (0.44, 0.44)] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(mx, 0.98, mz), Vec3::new(0.36, 1.64, 0.36), [15, 23, 42]));
                out.push(BoxPart::axis_aligned(c + Vec3::new(mx, 0.98, mz * 1.12), Vec3::new(0.08, 1.42, 0.06), [34, 211, 238]));
            }
            out.push(BoxPart::oriented(
                c + Vec3::new(0.0, 1.15 + (time_s * 2.5).sin() * 0.10, 0.0),
                Vec3::new(0.42, 0.42, 0.42),
                Quat::from_rotation_y(time_s * 1.8) * Quat::from_rotation_x(0.6),
                [56, 235, 255],
            ));
        }

        ModuleKind::MaterialsLabRig => {
            // 4-column hydraulic tensile press + moving crosshead + glowing test crystal
            for (cx, cz) in [(-0.42f32, -0.32f32), (0.42, -0.32), (-0.42, 0.32), (0.42, 0.32)] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(cx, 0.84, cz), Vec3::new(0.10, 1.36, 0.10), [148, 163, 184]));
            }
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 1.50, 0.0), Vec3::new(1.04, 0.14, 0.82), [99, 102, 241]));
            let press_y = 0.98 + (time_s * 2.2).sin() * 0.16;
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, press_y, 0.0), Vec3::new(0.92, 0.14, 0.64), [129, 140, 248]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.56, 0.0), Vec3::new(0.24, 0.34, 0.24), [56, 235, 255]));
        }

        ModuleKind::HoloBlueprintBench => {
            // Beveled lab projection table + 4 corner optical lenses + dual rotating holo rings
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.46, 0.0), Vec3::new(1.42, 0.60, 1.42), [30, 58, 95]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.78, 0.0), Vec3::new(1.16, 0.06, 1.16), [45, 212, 191]));
            out.push(BoxPart::oriented(
                c + Vec3::new(0.0, 1.24, 0.0),
                Vec3::new(0.58, 0.10, 0.58),
                Quat::from_rotation_y(time_s * 1.5) * Quat::from_rotation_z(0.35),
                [94, 234, 212],
            ));
        }

        ModuleKind::HoloBoardroomTable => {
            // Executive conference table + 6 induction dock stools + spinning 3D holographic factory twin & bar chart
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.48, 0.0), Vec3::new(1.76, 0.14, 1.16), [88, 28, 135]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.28, 0.0), Vec3::new(1.10, 0.32, 0.62), [51, 65, 85]));
            for sx in [-0.55f32, 0.0, 0.55] {
                for sz in [-0.74f32, 0.74] {
                    out.push(BoxPart::axis_aligned(c + Vec3::new(sx, 0.26, sz), Vec3::new(0.22, 0.24, 0.22), [250, 204, 21]));
                }
            }
            out.push(BoxPart::oriented(
                c + Vec3::new(0.0, 0.96, 0.0),
                Vec3::new(0.46, 0.36, 0.46),
                Quat::from_rotation_y(time_s * 1.3),
                [56, 235, 255],
            ));
        }

        ModuleKind::ContractDispatchTerminal => {
            // Curved triple-screen trading desk + satellite uplink mast
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.44, 0.12), Vec3::new(1.36, 0.56, 0.68), [51, 65, 85]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.0, 0.94, -0.12), Vec3::new(0.54, 0.36, 0.06), [56, 189, 248]));
            out.push(BoxPart::oriented(c + Vec3::new(-0.48, 0.94, 0.02), Vec3::new(0.46, 0.36, 0.06), Quat::from_rotation_y(0.45), [250, 204, 21]));
            out.push(BoxPart::oriented(c + Vec3::new(0.48, 0.94, 0.02), Vec3::new(0.46, 0.36, 0.06), Quat::from_rotation_y(-0.45), [52, 211, 153]));
        }

        ModuleKind::SupplyHedgeServer => {
            // Twin enterprise server racks + attached Chrome Lubricant-Espresso Dispenser on F3!
            for rx in [-0.34f32, 0.12] {
                out.push(BoxPart::axis_aligned(c + Vec3::new(rx, 0.90, 0.0), Vec3::new(0.42, 1.48, 0.68), [30, 41, 59]));
                out.push(BoxPart::axis_aligned(c + Vec3::new(rx, 0.95, 0.36), Vec3::new(0.30, 1.18, 0.04), [52, 211, 153]));
            }
            // Robot Lubricant-Espresso Bar sidecar
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.56, 0.58, 0.0), Vec3::new(0.34, 0.84, 0.48), [203, 213, 225]));
            out.push(BoxPart::axis_aligned(c + Vec3::new(0.56, 1.12, 0.0), Vec3::new(0.24, 0.24, 0.24), [245, 158, 11]));
        }

        ModuleKind::VerticalFreightLift => {}
    }
}
