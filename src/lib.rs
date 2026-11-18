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
//! # SwarmForge (`swarmforge` v1.3.0)
//!
//! Native safe-Rust 3D isometric automated robotic factory simulation with
//! industrial disasters & fire suppression, routine robot maintenance & repair bays,
//! cheering motivation drones, periodic outbound cargo drone sorties, progressive
//! warehouse architectural flare, indoor micro-drones, and tri-grid A* swarm navigation.

pub mod app;
pub mod assembly;
pub mod builder_ai;
pub mod disasters;
pub mod floors;
pub mod icon;
pub mod iso_renderer;
pub mod model;
pub mod models_3d;
pub mod navigation;
pub mod persistence;
pub mod swarm_tech;
