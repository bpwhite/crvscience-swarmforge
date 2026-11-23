# SwarmForge v1.3.0 User Guide

SwarmForge is a safe-Rust (`#![forbid(unsafe_code)]`) 3D isometric automated factory simulator combining multi-floor logistics (`B1` Utilities, `F1` Robotic Assembly, `F2` Tech Research, `F3` Administration), autonomous A* swarm robots, industrial hazards, and SQLite/JSON persistence.

---

## 1. Factory Presets & Getting Started

At the top of the left **Command Dock**, click any of the three one-click factory presets to benchmark or start from different scales:

1. **Starter (`10x10` F1)**: Single `Mk-I Basic Bot`, `Universal Proto-Bench`, `DeliveryDock`, and baseline `B1`–`F3` support infrastructure.
2. **Multi-Floor (`14x14` F1)**: Complete Stage `0`–`5` pipeline (`Raw Material Intake` -> `CNC Fabrication` -> `SMT Sub-Assembly` -> `Final Assembly` -> `Laser QA & Packaging` -> `Autonomous Delivery Dock`), `BotRepairStation`, `FireSuppressionTower`, `11` specialized robots across `B1`–`F3`, and Tier-2 `Autonomous Drone Core` unlocks.
3. **Megafactory Stress (`26x26` F1)**: Full-extent slab (`main_floor_half_extent = 13`) with parallel redundant assembly lines, `23` robots across all `9` classes, dual fire towers, and stress-tested corridor routing.

---

## 2. Camera, Floor Views & Visual Overlays

### Isometric Camera Controls
- **Orbit**: `Primary-Drag` inside the 3D viewport, or press `Q` / `E` to rotate yaw `-90°` / `+90°`.
- **Pan**: `Secondary-Drag` inside the viewport, or hold `W` / `A` / `S` / `D`.
- **Zoom**: Scroll wheel (`6.0x` to `95.0x`).
- **Reset Camera**: Press `R` or click **Reset Cam (R)** in the top HUD to restore default yaw, pitch, zoom, pan, and `F1` floor focus.
- **Compass Triad**: A depth-sorted `X` / `Y` / `Z` 3D axis indicator in the lower-left corner tracks camera orientation.

### Floor Switching & Cutaway Mode
- Press `1` (`B1` Basement), `2` (`F1` Main Floor), `3` (`F2` Tech Research), `4` (`F3` Administration), or `0` (`4-Floor Cutaway` translucent X-Ray stack).

### Toggleable Floor Overlays
In the top HUD under **Overlays**:
- **Cryo**: Shows dashed `R = 10.5u` coverage rings around every `FireSuppressionTower` on `F1`.
- **Cheer**: Shows dashed `R = 8.5u` morale boost rings around every airborne `MotivationDrone` on `F1`.
- **Heat**: Shows translucent amber/red corridor congestion tiles derived from `congestion_map`.
- **Drone Contact Shadows**: Airborne `MotivationDrone` and `DeliveryDock` cargo sortie drones cast altitude-scaled ground shadows directly onto `F1`.

---

## 3. Robot Fleet, Modules & Continuous Placement

### Robot Classes (`Robots` Tab)
Hover any robot card to view its speed (`u/s`), grid draw (`0.65 kW`), floor assignment, and role:
- **Floor 1 (`F1`)**: `Mk-I Basic Bot` (`2.05 u/s`), `Assembler Bot` (`2.55 u/s`), `AGV Hauler Bot` (`3.55 u/s`), `Builder Architect Bot` (`2.80 u/s`), `Field Repair & Fire Bot` (`3.15 u/s`), and `CheerBot Motivation Drone` (`3.40 u/s`, `+22%` cadence aura within `8.5u`).
- **Basement (`B1`)**: `Grid Maintenance Bot` (`2.40 u/s`).
- **Tech Labs (`F2`)**: `Lab Research Bot` (`2.25 u/s`).
- **Admin (`F3`)**: `Admin Executive Bot` (`2.15 u/s`).

### Placing Modules & Continuous Shift-Click Mode (`Modules` Tab)
1. Hover any module entry to inspect its **Pipeline Stage (`0–5` or Support)**, **Power Draw / Supply (`kW`)**, **Base Cost**, and special radii (`10.5u` suppression radius, repair tune-up rate).
2. Place via:
   - **Auto**: Places in an open spaced bay (`MIN_MODULE_CHEBYSHEV = 3` clearance).
   - **`@(gx,gz)`**: Places at the exact manual grid bay shown in the spinboxes.
   - **Click Floor**: Arms interactive 3D ghost-bay placement.
     - **Single Click**: Places one module and exits placement mode.
     - **Shift + Click**: Places a module and **keeps placement armed** for rapid multi-bay construction (`Esc` or right-click cancels).

---

## 4. Tech Tree, Disasters & Persistence

- **Swarm Intelligence (`S1–S8`) & Robotics (`R1–R8`)**: Unlock `[R8] Lights-Out Megafactory` to allow stations to continue production even while `Builder` bots relocate them (`being_relocated = true`).
- **Disasters & Suppression**: Under power overload or high facility scale, continuous-time Poisson hazards (`ThermalFire`, `GridOverload`, `CoolantLeak`, `LogicGlitch`) can erupt. Unsuppressed hazards escalate in severity until extinguished by `FireSuppressionTower`s, `RepairBot`s, or the top-HUD **[!] Suppress** override. **Pulse Sell** immediately dispatches 1 unit of the active product tier for quick capital.
- **Shortcuts Cheat Sheet (`F1`)**: Press `F1` (or click **About**) and select **Shortcuts Cheat Sheet** for an in-app reference grid.
- **SQLite WAL & JSON Snapshots (`Save/DB` Tab)**: Save/load/delete named slots in SQLite WAL (`swarmforge_saves.db`) or import/export full JSON state snapshots.

---

## 5. License

Copyright (C) 2026 CRV Science Network. Licensed under **GPL-3.0-or-later** (`LICENSE`).
