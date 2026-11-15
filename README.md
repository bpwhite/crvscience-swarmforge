# SwarmForge (`swarmforge` v1.3.0)

Native safe-Rust (`#![forbid(unsafe_code)]`) 3D isometric automated robotic factory simulation game featuring **bespoke articulated 3D CAD models**, **industrial disasters & fire suppression turrets**, **routine robot maintenance (`RepairBot` + `BotRepairStation`)**, **overhead cheering `MotivationDrone`s**, **periodic outbound cargo drone sorties**, **progressive 6-tier non-obstructing warehouse flare**, **indoor ambient micro-drones**, and **tri-grid (`0.8u`) A* collision-free navigation**.

## What's New in v1.3.0
- **Disaster & Fire Suppression Engine (`src/disasters.rs`)**: Thermal Station Fires, HV Arc-Flash Overloads, Cryo Coolant Ruptures, and Firmware Logic Glitches with automatic `FireSuppressionTower` cryo-foam streams (`R = 10.5u`), `RepairBot` emergency response, and HUD manual suppression/test-drill controls.
- **Routine Robot Maintenance (`RepairBot` + `BotRepairStation`)**: Working bots gradually accumulate mechanical wear (`integrity`); bots autonomously pit-stop at `BotRepairStation` diagnostic bays (`+100%` tune-up + temporary overclock boost) or receive mobile in-field repairs from `RepairBot`s.
- **Periodic `DeliveryDock` Cargo Drone Fly-Offs**: Outbound quad-rotor cargo drones latch product pods underneath, climb vertically, bank across the sky beyond the factory perimeter, and return for precision helipad landings.
- **`CheerBot` Motivation Drone (`MotivationDrone`)**: Non-obstructing coaxial aerial unit cruising overhead (`y = 2.6u`), broadcasting periodic cheer bursts (`+22%` servo cadence & movement morale aura).
- **Continually Evolving Warehouse Flare & Indoor Micro-Drones**: 6 progressive non-obstructing perimeter & rafter architectural tiers (safety bollards, sodium/LED floodlight towers, digital production scoreboards, overhead cyan busbar trusses, spinning holo-pylons, golden monument) plus cruising indoor LiDAR & micro-courier drones.

## Quick Start

```bash
cargo test
cargo run --release
cargo run --release -- --headless-ticks 600 --export-json swarmforge_save.json
```
