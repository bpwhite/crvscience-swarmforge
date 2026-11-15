#![forbid(unsafe_code)]
//! v1.2.2 Exact Tri-Grid Navigation Engine:
//! - `CELL_PITCH = 2.4`, `NAV_STEP = 0.8` (exact 3 sub-cells per bay!)
//! - `MODULE_COLLISION_HALF = 0.85`, max `robot_radius = 0.44` -> max solid keepout `1.29u`
//! - Sub-cells `±2 * 0.8 = ±1.60u` and `±3 * 0.8 = ±2.40u` give EVERY robot class (including Builder!)
//!   a 3-node-wide unblocked corridor between any two `chebyshev >= 2` modules (`4.8u` center-to-center)
//! - Station aprons sit at exact A* grid nodes (`±1.60u`)
//! - `find_safe_open_point` sanitizes all robot spawns, Idle patrols, and loaded saves so targets NEVER land inside modules

use crate::model::{FactoryModule, FloorId, GameState, Robot, RobotKind, RobotTask};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

pub const CELL_PITCH: f32 = 2.4;
pub const NAV_STEP: f32 = 0.8; // Exact 1/3 bay pitch -> 3 open A* lanes in every module corridor!
pub const MIN_MODULE_CHEBYSHEV: i32 = 2;
pub const MODULE_COLLISION_HALF: f32 = 0.85;

pub fn robot_radius(kind: RobotKind) -> f32 {
    match kind {
        RobotKind::Hauler => 0.44,
        RobotKind::Builder => 0.44,
        RobotKind::Maintenance => 0.42,
        RobotKind::RepairBot => 0.41,
        RobotKind::Assembler => 0.40,
        RobotKind::BasicWorker => 0.39,
        RobotKind::Researcher => 0.38,
        RobotKind::Administrator => 0.38,
        RobotKind::MotivationDrone => 0.0, // Airborne at y = 2.6u!
    }
}

pub fn has_module_clearance(
    modules: &[FactoryModule],
    floor: FloorId,
    gx: i32,
    gz: i32,
    ignore_module_id: Option<u32>,
) -> bool {
    if floor != FloorId::MainFloor
        && (gx - (-3)).abs().max((gz - (-3)).abs()) < MIN_MODULE_CHEBYSHEV
    {
        return false;
    }
    for m in modules {
        if m.floor != floor || Some(m.id) == ignore_module_id {
            continue;
        }
        let cheb = (m.gx - gx).abs().max((m.gz - gz).abs());
        if cheb < MIN_MODULE_CHEBYSHEV {
            return false;
        }
    }
    true
}

pub fn point_hits_obstacle(
    modules: &[FactoryModule],
    floor: FloorId,
    wx: f32,
    wz: f32,
    clearance: f32,
) -> bool {
    let keep = MODULE_COLLISION_HALF + clearance;
    if floor != FloorId::MainFloor {
        let sx = -3.0 * CELL_PITCH;
        let sz = -3.0 * CELL_PITCH;
        if (wx - sx).abs() < keep && (wz - sz).abs() < keep {
            return true;
        }
    }
    for m in modules {
        if m.floor != floor || m.being_relocated {
            continue;
        }
        if (wx - m.visual_x).abs() < keep && (wz - m.visual_z).abs() < keep {
            return true;
        }
    }
    false
}

/// Snaps any desired world coordinate `(wx, wz)` (spawn point, patrol point, escort point, or legacy save)
/// to the nearest strictly obstacle-free corridor point on `floor`.
pub fn find_safe_open_point(
    modules: &[FactoryModule],
    floor: FloorId,
    half_extent: i32,
    wx: f32,
    wz: f32,
    clearance: f32,
) -> (f32, f32) {
    let limit = half_extent as f32 * CELL_PITCH + 0.2;
    let cx0 = wx.clamp(-limit, limit);
    let cz0 = wz.clamp(-limit, limit);
    if !point_hits_obstacle(modules, floor, cx0, cz0, clearance) {
        return (cx0, cz0);
    }

    let bx = (cx0 / NAV_STEP).round() as i32;
    let bz = (cz0 / NAV_STEP).round() as i32;
    let max_n = (limit / NAV_STEP).floor() as i32;

    let mut best = (cx0, cz0);
    let mut best_d = f32::MAX;
    for r in 1..=12 {
        for dx in -r..=r {
            for dz in -r..=r {
                let nx = (bx + dx).clamp(-max_n, max_n);
                let nz = (bz + dz).clamp(-max_n, max_n);
                let px = nx as f32 * NAV_STEP;
                let pz = nz as f32 * NAV_STEP;
                if !point_hits_obstacle(modules, floor, px, pz, clearance) {
                    let d = (px - cx0).hypot(pz - cz0);
                    if d < best_d {
                        best_d = d;
                        best = (px, pz);
                    }
                }
            }
        }
        if best_d < f32::MAX {
            return best;
        }
    }
    best
}

pub fn enforce_spacing_and_migrate(state: &mut GameState) {
    if state.main_floor_half_extent < 5 {
        state.main_floor_half_extent = 5;
    }

    let mut placed: Vec<(FloorId, i32, i32, u32)> = Vec::new();
    for i in 0..state.modules.len() {
        let floor = state.modules[i].floor;
        let id = state.modules[i].id;
        let orig_gx = state.modules[i].gx;
        let orig_gz = state.modules[i].gz;

        let shaft_ok = floor == FloorId::MainFloor
            || (orig_gx + 3).abs().max((orig_gz + 3).abs()) >= MIN_MODULE_CHEBYSHEV;
        let valid_here = shaft_ok
            && placed.iter().all(|&(pf, px, pz, _)| {
                pf != floor || (px - orig_gx).abs().max((pz - orig_gz).abs()) >= MIN_MODULE_CHEBYSHEV
            });

        let (final_gx, final_gz) = if valid_here {
            (orig_gx, orig_gz)
        } else {
            let mut found = None;
            for radius in 1..=20 {
                let half = if floor == FloorId::MainFloor {
                    state.main_floor_half_extent
                } else {
                    5
                };
                for dx in -radius..=radius {
                    for dz in -radius..=radius {
                        let cx = orig_gx + dx;
                        let cz = orig_gz + dz;
                        if cx < -half || cx >= half || cz < -half || cz >= half {
                            continue;
                        }
                        if floor != FloorId::MainFloor
                            && (cx + 3).abs().max((cz + 3).abs()) < MIN_MODULE_CHEBYSHEV
                        {
                            continue;
                        }
                        let ok = placed.iter().all(|&(pf, px, pz, _)| {
                            pf != floor
                                || (px - cx).abs().max((pz - cz).abs()) >= MIN_MODULE_CHEBYSHEV
                        });
                        if ok {
                            found = Some((cx, cz));
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
                if floor == FloorId::MainFloor && state.main_floor_half_extent < 13 {
                    state.main_floor_half_extent += 1;
                }
            }
            found.unwrap_or((orig_gx, orig_gz))
        };

        state.modules[i].gx = final_gx;
        state.modules[i].gz = final_gz;
        if !state.modules[i].being_relocated {
            state.modules[i].visual_x = final_gx as f32 * CELL_PITCH;
            state.modules[i].visual_z = final_gz as f32 * CELL_PITCH;
        }
        placed.push((floor, final_gx, final_gz, id));
    }

    // Sanitize all existing robots & their targets out of any module plinths!
    let half_f1 = state.main_floor_half_extent;
    for r in &mut state.robots {
        let half = match r.floor {
            FloorId::MainFloor => half_f1,
            FloorId::Administration => 4,
            _ => 5,
        };
        let rad = robot_radius(r.kind) + 0.08;
        let (sx, sz) = find_safe_open_point(&state.modules, r.floor, half, r.x, r.z, rad);
        r.x = sx;
        r.z = sz;
        let (tx, tz) = find_safe_open_point(&state.modules, r.floor, half, r.target_x, r.target_z, rad);
        r.target_x = tx;
        r.target_z = tz;
    }

    for f in FloorId::ALL {
        resolve_floor_collisions(state, f);
    }
}

/// Picks an approach-aware exterior docking point at `apron = 1.60u` (`2 * NAV_STEP`),
/// landing dead-on an unblocked tri-grid A* node outside `m`'s collision box!
pub fn module_dock_point_from(
    m: &FactoryModule,
    robot_id: u32,
    from_x: f32,
    from_z: f32,
    modules: &[FactoryModule],
    robots: &[Robot],
    half_extent: i32,
) -> (f32, f32) {
    let apron = 1.60f32; // Exact 2 * NAV_STEP = 1.60u (> 0.85 + 0.44 = 1.29u)
    let candidates = [
        (apron, 0.0),
        (-apron, 0.0),
        (0.0, apron),
        (0.0, -apron),
        (apron, apron),
        (apron, -apron),
        (-apron, apron),
        (-apron, -apron),
    ];
    let limit = half_extent as f32 * CELL_PITCH + 0.25;

    let mut best = (m.visual_x + apron, m.visual_z);
    let mut best_score = f32::MAX;

    for (idx, &(ox, oz)) in candidates.iter().enumerate() {
        let px = (m.visual_x + ox).clamp(-limit, limit);
        let pz = (m.visual_z + oz).clamp(-limit, limit);

        if point_hits_obstacle(modules, m.floor, px, pz, 0.50) {
            continue;
        }

        let approach_dist = (px - from_x).hypot(pz - from_z);
        let crowd: usize = robots
            .iter()
            .filter(|r| {
                r.id != robot_id
                    && r.floor == m.floor
                    && ((r.x - px).powi(2) + (r.z - pz).powi(2)) < 0.85
            })
            .count();
        let tie = (((robot_id as usize + idx) % 8) as f32) * 0.06;
        let score = approach_dist + (crowd as f32) * 3.0 + tie;
        if score < best_score {
            best_score = score;
            best = (px, pz);
        }
    }
    best
}

pub fn module_dock_point(
    m: &FactoryModule,
    robot_id: u32,
    modules: &[FactoryModule],
    robots: &[Robot],
    half_extent: i32,
) -> (f32, f32) {
    let (fx, fz) = robots
        .iter()
        .find(|r| r.id == robot_id)
        .map(|r| (r.x, r.z))
        .unwrap_or((0.0, 0.0));
    module_dock_point_from(m, robot_id, fx, fz, modules, robots, half_extent)
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct AStarNode {
    f_cost: i32,
    g_cost: i32,
    nx: i32,
    nz: i32,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_cost.cmp(&self.f_cost)
    }
}
impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn astar_corridor_cost(
    modules: &[FactoryModule],
    congestion: &HashMap<(i32, i32), f32>,
    floor: FloorId,
    half_extent: i32,
    start: (i32, i32),
    goal: (i32, i32),
) -> (f32, Vec<(i32, i32)>) {
    if start == goal {
        return (0.0, vec![start]);
    }
    let blocked = |gx: i32, gz: i32| -> bool {
        if (gx, gz) == start || (gx, gz) == goal {
            return false;
        }
        modules
            .iter()
            .any(|m| m.floor == floor && !m.being_relocated && m.gx == gx && m.gz == gz)
    };

    let min_b = -half_extent - 1;
    let max_b = half_extent + 1;
    let mut open = BinaryHeap::new();
    let mut g_map: HashMap<(i32, i32), i32> = HashMap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();

    let h = |x: i32, z: i32| -> i32 { ((x - goal.0).abs() + (z - goal.1).abs()) * 10 };
    g_map.insert(start, 0);
    open.push(AStarNode {
        f_cost: h(start.0, start.1),
        g_cost: 0,
        nx: start.0,
        nz: start.1,
    });

    let dirs = [
        (1, 0, 10),
        (-1, 0, 10),
        (0, 1, 10),
        (0, -1, 10),
        (1, 1, 14),
        (1, -1, 14),
        (-1, 1, 14),
        (-1, -1, 14),
    ];

    let mut expansions = 0usize;
    while let Some(cur) = open.pop() {
        if (cur.nx, cur.nz) == goal {
            let mut path = vec![goal];
            let mut p = goal;
            while let Some(&prev) = came_from.get(&p) {
                path.push(prev);
                if prev == start {
                    break;
                }
                p = prev;
            }
            path.reverse();
            return (cur.g_cost as f32 / 10.0, path);
        }
        expansions += 1;
        if expansions > 900 {
            break;
        }
        if cur.g_cost > *g_map.get(&(cur.nx, cur.nz)).unwrap_or(&i32::MAX) {
            continue;
        }

        for &(dx, dz, base_step) in &dirs {
            let nx = cur.nx + dx;
            let nz = cur.nz + dz;
            if nx < min_b || nx > max_b || nz < min_b || nz > max_b || blocked(nx, nz) {
                continue;
            }
            if dx != 0 && dz != 0 && (blocked(cur.nx + dx, cur.nz) || blocked(cur.nx, cur.nz + dz))
            {
                continue;
            }
            let heat_pen = (congestion.get(&(nx, nz)).copied().unwrap_or(0.0) * 12.0) as i32;
            let next_g = cur.g_cost + base_step + heat_pen;
            if next_g < *g_map.get(&(nx, nz)).unwrap_or(&i32::MAX) {
                g_map.insert((nx, nz), next_g);
                came_from.insert((nx, nz), (cur.nx, cur.nz));
                open.push(AStarNode {
                    f_cost: next_g + h(nx, nz),
                    g_cost: next_g,
                    nx,
                    nz,
                });
            }
        }
    }

    let fallback = ((start.0 - goal.0).abs() + (start.1 - goal.1).abs()) as f32 * 3.0 + 12.0;
    (fallback, vec![start, goal])
}

/// Tri-grid (`NAV_STEP = 0.8`) A* router where every module corridor has 3 open lanes (`±1.6, ±2.4`)
/// for ALL robot classes including heavy Builder and Hauler bots!
fn fine_next_waypoint(
    modules: &[FactoryModule],
    floor: FloorId,
    half_extent: i32,
    rx: f32,
    rz: f32,
    tx: f32,
    tz: f32,
) -> (f32, f32) {
    let fine_half = half_extent * 3 + 2;
    // 0.85 + 0.46 = 1.31u -> blocks 0.0 and ±0.8, leaves ±1.6 and ±2.4 wide open!
    let is_blocked = |nx: i32, nz: i32| -> bool {
        point_hits_obstacle(modules, floor, nx as f32 * NAV_STEP, nz as f32 * NAV_STEP, 0.46)
    };

    let snap_free = |wx: f32, wz: f32| -> (i32, i32) {
        let bx = (wx / NAV_STEP).round() as i32;
        let bz = (wz / NAV_STEP).round() as i32;
        if !is_blocked(bx, bz) {
            return (bx, bz);
        }
        let mut best = (bx, bz);
        let mut best_d = f32::MAX;
        for r in 1..=6 {
            for dx in -r..=r {
                for dz in -r..=r {
                    let cx = bx + dx;
                    let cz = bz + dz;
                    if !is_blocked(cx, cz) {
                        let d = (cx as f32 * NAV_STEP - wx).hypot(cz as f32 * NAV_STEP - wz);
                        if d < best_d {
                            best_d = d;
                            best = (cx, cz);
                        }
                    }
                }
            }
            if best_d < f32::MAX {
                break;
            }
        }
        best
    };

    let start = snap_free(rx, rz);
    let goal = snap_free(tx, tz);
    if start == goal {
        return (tx, tz);
    }

    let mut open = BinaryHeap::new();
    let mut g_map: HashMap<(i32, i32), i32> = HashMap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let h = |x: i32, z: i32| -> i32 { ((x - goal.0).abs() + (z - goal.1).abs()) * 10 };

    g_map.insert(start, 0);
    open.push(AStarNode {
        f_cost: h(start.0, start.1),
        g_cost: 0,
        nx: start.0,
        nz: start.1,
    });

    let dirs = [
        (1, 0, 10),
        (-1, 0, 10),
        (0, 1, 10),
        (0, -1, 10),
        (1, 1, 14),
        (1, -1, 14),
        (-1, 1, 14),
        (-1, -1, 14),
    ];

    let mut steps = 0usize;
    while let Some(cur) = open.pop() {
        if (cur.nx, cur.nz) == goal {
            let mut path = vec![goal];
            let mut p = goal;
            while let Some(&prev) = came_from.get(&p) {
                path.push(prev);
                if prev == start {
                    break;
                }
                p = prev;
            }
            path.reverse();
            let pick = if path.len() >= 3 { path[2] } else { path[1] };
            return (pick.0 as f32 * NAV_STEP, pick.1 as f32 * NAV_STEP);
        }
        steps += 1;
        if steps > 2200 {
            break;
        }
        if cur.g_cost > *g_map.get(&(cur.nx, cur.nz)).unwrap_or(&i32::MAX) {
            continue;
        }

        for &(dx, dz, step_cost) in &dirs {
            let nx = cur.nx + dx;
            let nz = cur.nz + dz;
            if nx < -fine_half || nx > fine_half || nz < -fine_half || nz > fine_half {
                continue;
            }
            if is_blocked(nx, nz) {
                continue;
            }
            if dx != 0 && dz != 0 && (is_blocked(cur.nx + dx, cur.nz) || is_blocked(cur.nx, cur.nz + dz)) {
                continue;
            }
            let ng = cur.g_cost + step_cost;
            if ng < *g_map.get(&(nx, nz)).unwrap_or(&i32::MAX) {
                g_map.insert((nx, nz), ng);
                came_from.insert((nx, nz), (cur.nx, cur.nz));
                open.push(AStarNode {
                    f_cost: ng + h(nx, nz),
                    g_cost: ng,
                    nx,
                    nz,
                });
            }
        }
    }
    (tx, tz)
}

fn segment_hits_any_obstacle(
    modules: &[FactoryModule],
    floor: FloorId,
    x0: f32,
    z0: f32,
    x1: f32,
    z1: f32,
    pad: f32,
) -> bool {
    let seg_dx = x1 - x0;
    let seg_dz = z1 - z0;
    let len2 = seg_dx * seg_dx + seg_dz * seg_dz;
    if len2 < 1e-5 {
        return false;
    }
    let keep = MODULE_COLLISION_HALF + pad;

    let check_box = |bx: f32, bz: f32| -> bool {
        let t = (((bx - x0) * seg_dx + (bz - z0) * seg_dz) / len2).clamp(0.0, 1.0);
        let cx = x0 + seg_dx * t;
        let cz = z0 + seg_dz * t;
        (cx - bx).abs() < keep && (cz - bz).abs() < keep
    };

    if floor != FloorId::MainFloor && check_box(-3.0 * CELL_PITCH, -3.0 * CELL_PITCH) {
        return true;
    }
    for m in modules {
        if m.floor != floor || m.being_relocated {
            continue;
        }
        if check_box(m.visual_x, m.visual_z) {
            return true;
        }
    }
    false
}

pub fn step_robot_with_collision(
    state: &mut GameState,
    r_idx: usize,
    speed: f32,
    dt: f32,
) -> bool {
    let floor = state.robots[r_idx].floor;
    let half_ext = match floor {
        FloorId::MainFloor => state.main_floor_half_extent,
        FloorId::Administration => 4,
        _ => 5,
    };
    let rad = robot_radius(state.robots[r_idx].kind);

    // Guarantee `target` itself is NEVER inside a solid module plinth!
    if point_hits_obstacle(
        &state.modules,
        floor,
        state.robots[r_idx].target_x,
        state.robots[r_idx].target_z,
        rad + 0.06,
    ) {
        let (safe_tx, safe_tz) = find_safe_open_point(
            &state.modules,
            floor,
            half_ext,
            state.robots[r_idx].target_x,
            state.robots[r_idx].target_z,
            rad + 0.12,
        );
        state.robots[r_idx].target_x = safe_tx;
        state.robots[r_idx].target_z = safe_tz;
    }

    let rx = state.robots[r_idx].x;
    let rz = state.robots[r_idx].z;
    let tx = state.robots[r_idx].target_x;
    let tz = state.robots[r_idx].target_z;
    let bot_id = state.robots[r_idx].id;

    let dx_goal = tx - rx;
    let dz_goal = tz - rz;
    let dist_goal = dx_goal.hypot(dz_goal);
    if dist_goal <= 0.34 {
        return true;
    }

    let (wp_x, wp_z) = if segment_hits_any_obstacle(&state.modules, floor, rx, rz, tx, tz, rad + 0.08) {
        fine_next_waypoint(&state.modules, floor, half_ext, rx, rz, tx, tz)
    } else {
        (tx, tz)
    };

    let wp_d = (wp_x - rx).hypot(wp_z - rz).max(1e-4);
    let mut dir_x = (wp_x - rx) / wp_d;
    let mut dir_z = (wp_z - rz) / wp_d;

    let slide_shell = MODULE_COLLISION_HALF + rad + 0.28;
    let mut slide_obstacles: Vec<(f32, f32)> = state
        .modules
        .iter()
        .filter(|m| m.floor == floor && !m.being_relocated)
        .map(|m| (m.visual_x, m.visual_z))
        .collect();
    if floor != FloorId::MainFloor {
        slide_obstacles.push((-3.0 * CELL_PITCH, -3.0 * CELL_PITCH));
    }

    for (mx, mz) in slide_obstacles {
        let ox = rx - mx;
        let oz = rz - mz;
        if ox.abs() < slide_shell && oz.abs() < slide_shell {
            let d = ox.hypot(oz).max(1e-3);
            let nx = ox / d;
            let nz = oz / d;
            let into_wall = -(dir_x * nx + dir_z * nz);
            if into_wall > -0.10 {
                let t1 = (-nz, nx);
                let t2 = (nz, -nx);
                let g_dx = dx_goal / dist_goal;
                let g_dz = dz_goal / dist_goal;
                let dot1 = t1.0 * g_dx + t1.1 * g_dz;
                let dot2 = t2.0 * g_dx + t2.1 * g_dz;
                let (tx_s, tz_s) = if (dot1 - dot2).abs() < 0.08 {
                    if bot_id % 2 == 0 { t1 } else { t2 }
                } else if dot1 > dot2 {
                    t1
                } else {
                    t2
                };
                let boost = (into_wall + 0.35).max(0.45) * 1.65;
                dir_x += tx_s * boost + nx * 0.55;
                dir_z += tz_s * boost + nz * 0.55;
            }
        }
    }

    let peloton = state.has_tech("swarm_peloton");
    let mut yield_factor = 1.0f32;
    let mut had_crowding = false;

    for (o_idx, other) in state.robots.iter().enumerate() {
        if o_idx == r_idx || other.floor != floor {
            continue;
        }
        let ox = other.x - rx;
        let oz = other.z - rz;
        let d = ox.hypot(oz);
        let min_sep = rad + robot_radius(other.kind);
        let lookahead = min_sep + 0.90;
        if d > 1e-4 && d < lookahead {
            let nx = ox / d;
            let nz = oz / d;
            if dir_x * nx + dir_z * nz > -0.15 {
                let urgency = ((lookahead - d) / 0.90).clamp(0.0, 1.4);
                let side_strength = if peloton { 1.35 } else { 1.10 };
                dir_x += (-nz * side_strength - nx * 0.70) * urgency;
                dir_z += (nx * side_strength - nz * 0.70) * urgency;
                if !peloton {
                    yield_factor = yield_factor.min((1.0 - urgency * 0.30).max(0.50));
                }
                had_crowding = true;
            }
        }
    }

    {
        let entry = state.stuck_tracker.entry(bot_id).or_insert((rx, rz, 0.0));
        let moved = (rx - entry.0).hypot(rz - entry.1);
        if moved > 0.16 {
            *entry = (rx, rz, 0.0);
        } else {
            entry.2 += dt;
            if entry.2 > 0.50 {
                let sign = if (bot_id + (state.tick_count as u32 / 15)) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                };
                dir_x = -dir_z * sign * 2.2 + dir_x * 0.3;
                dir_z = dir_x * sign * 2.2 + dir_z * 0.3;
                if entry.2 > 1.05 {
                    *entry = (rx, rz, 0.0);
                }
            }
        }
    }

    let norm = dir_x.hypot(dir_z).max(1e-4);
    dir_x /= norm;
    dir_z /= norm;

    let step = (speed * yield_factor * dt).min(dist_goal);
    state.robots[r_idx].x += dir_x * step;
    state.robots[r_idx].z += dir_z * step;
    state.robots[r_idx].heading = dir_z.atan2(dir_x);

    if had_crowding && floor == FloorId::MainFloor {
        let cg = (
            (state.robots[r_idx].x / CELL_PITCH).round() as i32,
            (state.robots[r_idx].z / CELL_PITCH).round() as i32,
        );
        let entry = state.congestion_map.entry(cg).or_insert(0.0);
        *entry = (*entry + dt * 1.8).min(8.0);
    }

    resolve_floor_collisions(state, floor);

    let fx = state.robots[r_idx].x;
    let fz = state.robots[r_idx].z;
    ((tx - fx).powi(2) + (tz - fz).powi(2)).sqrt() <= 0.36
}

pub fn resolve_floor_collisions(state: &mut GameState, floor: FloorId) {
    let indices: Vec<usize> = state
        .robots
        .iter()
        .enumerate()
        .filter(|(_, r)| r.floor == floor && r.kind != RobotKind::MotivationDrone)
        .map(|(i, _)| i)
        .collect();

    let slab_limit = match floor {
        FloorId::MainFloor => state.main_floor_half_extent as f32 * CELL_PITCH + 0.6,
        FloorId::Administration => 4.0 * CELL_PITCH,
        _ => 5.0 * CELL_PITCH,
    };

    for _iter in 0..4 {
        for a in 0..indices.len() {
            for b in (a + 1)..indices.len() {
                let ia = indices[a];
                let ib = indices[b];
                let dx = state.robots[ib].x - state.robots[ia].x;
                let dz = state.robots[ib].z - state.robots[ia].z;
                let min_d =
                    robot_radius(state.robots[ia].kind) + robot_radius(state.robots[ib].kind);
                let dist = dx.hypot(dz);
                if dist < min_d {
                    let (nx, nz) = if dist > 1e-4 {
                        (dx / dist, dz / dist)
                    } else {
                        let ang = (ia as f32) * 1.7 + (ib as f32) * 0.9;
                        (ang.cos(), ang.sin())
                    };
                    let push = (min_d - dist) * 0.52;
                    state.robots[ia].x -= nx * push;
                    state.robots[ia].z -= nz * push;
                    state.robots[ib].x += nx * push;
                    state.robots[ib].z += nz * push;
                }
            }
        }

        for &idx in &indices {
            let rad = robot_radius(state.robots[idx].kind);
            let is_relocating_this = match state.robots[idx].task {
                RobotTask::RelocatingModule { module_id, .. } => Some(module_id),
                _ => None,
            };

            let robot = &mut state.robots[idx];
            if floor != FloorId::MainFloor {
                push_circle_out_of_box(
                    &mut robot.x,
                    &mut robot.z,
                    rad,
                    -3.0 * CELL_PITCH,
                    -3.0 * CELL_PITCH,
                    MODULE_COLLISION_HALF,
                );
            }

            for m in &state.modules {
                if m.floor != floor || m.being_relocated || Some(m.id) == is_relocating_this {
                    continue;
                }
                push_circle_out_of_box(
                    &mut robot.x,
                    &mut robot.z,
                    rad,
                    m.visual_x,
                    m.visual_z,
                    MODULE_COLLISION_HALF,
                );
            }

            robot.x = robot.x.clamp(-slab_limit, slab_limit);
            robot.z = robot.z.clamp(-slab_limit, slab_limit);
        }
    }
}

fn push_circle_out_of_box(rx: &mut f32, rz: &mut f32, rad: f32, bx: f32, bz: f32, half: f32) {
    let cx = (*rx).clamp(bx - half, bx + half);
    let cz = (*rz).clamp(bz - half, bz + half);
    let dx = *rx - cx;
    let dz = *rz - cz;
    let d = dx.hypot(dz);
    if d < rad {
        if d > 1e-4 {
            let push = rad - d + 0.005;
            *rx += (dx / d) * push;
            *rz += (dz / d) * push;
        } else {
            let ox = *rx - bx;
            let oz = *rz - bz;
            if ox.abs() >= oz.abs() {
                *rx = bx + (if ox >= 0.0 { 1.0 } else { -1.0 }) * (half + rad + 0.03);
            } else {
                *rz = bz + (if oz >= 0.0 { 1.0 } else { -1.0 }) * (half + rad + 0.03);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModuleKind;

    #[test]
    fn builder_robot_traverses_dense_modules_and_spawns_safely() {
        let mut state = GameState::new_starter_factory();
        let mid = state.alloc_id();
        state.modules.push(FactoryModule::new(
            mid,
            ModuleKind::FabricationCell,
            FloorId::MainFloor,
            0,
            0,
        ));
        let bid = state.alloc_id();
        let (sx, sz) = find_safe_open_point(&state.modules, FloorId::MainFloor, 5, 0.0, 0.0, 0.52);
        state.robots.push(Robot {
            id: bid,
            kind: RobotKind::Builder,
            floor: FloorId::MainFloor,
            x: sx,
            z: sz,
            target_x: 1.60,
            target_z: 0.0,
            heading: 0.0,
            tier: 1,
            energy: 1.0,
            integrity: 1.0,
            morale_boost_timer: 0.0,
            task: RobotTask::Idle,
            tasks_completed: 0,
        });
        let idx = state.robots.len() - 1;
        let mut arrived = false;
        for _ in 0..100 {
            if step_robot_with_collision(&mut state, idx, 2.8, 0.05) {
                arrived = true;
                break;
            }
        }
        assert!(arrived, "Builder Bot failed to navigate around (0, 0)!");
    }
}
