# SwarmForge v1.3.0 Scientific Methods & Numerical Formulation

This document specifies the discrete-grid pathfinding, assembly queueing, stochastic hazard kinetics, and 3D isometric shading models implemented in SwarmForge (`GPL-3.0-or-later`).

---

## 1. Exact Admissible Octile A* Corridor Routing (`src/navigation.rs`)

SwarmForge evaluates 8-connected factory floor corridors on integer grid coordinates `(gx, gz)` spaced at `CELL_PITCH = 2.4` world units (coarse module grid) and `NAV_STEP = 0.8` world units (tri-grid robot collision router).

### 1.1 Step Cost & Octile Heuristic
Cardinal moves (`(±1, 0)`, `(0, ±1)`) incur cost `10` (representing `1.0` grid unit in tenths), and corner-cutting-free diagonal moves (`(±1, ±1)`) incur cost `14` (`≈ 10 * sqrt(2)`). For node `(x, z)` and goal `(gx, gz)` with `dx = |x - gx|` and `dz = |z - gz|`:

```text
hi = max(dx, dz)
lo = min(dx, dz)
h_octile(x, z) = 10 * hi + 4 * lo
```

Because `10 * (hi - lo) + 14 * lo = 10 * hi + 4 * lo`, `h_octile` equals the exact shortest unobstructed 8-connected grid distance in tenths. Unlike Manhattan distance `10 * (dx + dz)` (which overestimates diagonal moves by `~42.9%` and breaks A* admissibility), `h_octile` satisfies `h_octile(n) <= d*(n, goal)` everywhere, guaranteeing optimal paths across open and congested corridors.

### 1.2 Congestion Heat Coupling
Each candidate neighbor `(nx, nz)` adds a congestion penalty proportional to the decaying robot traffic density `C(nx, nz)`:

```text
g_next = g_cur + step_cost + floor(12.0 * C(nx, nz))
f(n)   = g(n) + h_octile(n)
```

Along the diagonal `(0,0) -> (4,4)` on an open floor (`C = 0`), `astar_corridor_cost` visits `5` cells with cumulative cost `4 * 14 / 10.0 = 5.6`.

---

## 2. Assembly Line Kinetics & Multi-Unit Substep Conservation (`src/assembly.rs`)

For each active production station on `MainFloor`, processing rate `R` (units/s) combines product difficulty, hazard derating, cooperative operators, swarm multipliers, power grid adequacy, coolant pressure, layout efficiency, and equipment condition:

```text
R = (0.55 / cycle_difficulty) * hazard_mult * coop_factor
    * mult.work_speed * grid_factor * pressure_bonus * layout_bonus * condition
```

where:
- `grid_factor = clamp(P_supply / P_demand, 0.35, 1.0)` when `P_demand > P_supply`, else `1.0`
- `pressure_bonus = clamp(P_coolant / 90.0, 0.85, 1.35)`
- `layout_bonus = 0.65 + 0.75 * layout_efficiency`
- `hazard_mult = 0.20` if an active disaster sits on the module, else `1.0`

### 2.1 Zero-Loss Multi-Unit Draining Loop
During high-speed ticks (`2x`, `4x`) or fast cooperative cells where `R * dt >= 1.0`, progress accumulates and drains all complete units while conserving fractional work:

```text
work_progress <- work_progress + R * dt
while work_progress >= 1.0 and input_buffer > 0 and output_buffer < 8:
    work_progress <- work_progress - 1.0
    input_buffer  <- input_buffer - 1
    output_buffer <- output_buffer + 1
```

### 2.2 Lights-Out Hot-Swap Continuity (`robo_lights_out`)
When `robo_lights_out` (`[R8]`) is unlocked, modules marked `being_relocated = true` remain included in active pipeline stages, input feedstock pulls, hauler/assembler dispatch, and assembly rate evaluation (`!m.being_relocated || lights_out`).

---

## 3. Continuous-Time Deterministic Poisson Hazards & Escalation (`src/disasters.rs`)

### 3.1 SplitMix64 Deterministic PRNG State
All stochastic disaster triggers and module selections advance a serialized 64-bit state `S = GameState::rng_seed` via SplitMix64:

```text
S_{k+1} = S_k + 0x9E3779B97F4A7C15  (mod 2^64)
z       = (S_{k+1} ^ (S_{k+1} >> 30)) * 0xBF58476D1CE4E5B9
z       = (z ^ (z >> 27)) * 0x94D049BB133111EB
out_k   = z ^ (z >> 31)
```

Uniform variates `U in [0, 1)` are extracted from the top 24 bits: `U = (out_k >> 40) / 2^24`.

### 3.2 Time-Step Invariant Poisson Hazard Rate
After warm-up (`elapsed_secs > 22.0`) and while `< 3` active disasters exist, instantaneous hazard intensity `lambda` (`s^-1`) scales with grid overload ratio and facility module count `N_mod`:

```text
overload_ratio = max(P_demand / max(P_supply, 1.0), 0.5)
lambda         = 0.0048 * overload_ratio * (1.0 + 0.025 * N_mod)
p_trigger(dt)  = 1 - exp(-lambda * dt)
```

Because `1 - (1 - p(dt / K))^K = p(dt)`, expected disaster frequency is independent of simulation step size `dt`.

### 3.3 Suppression vs. Unsuppressed Fire Escalation
Active disasters degrade module condition at `d(condition)/dt = -0.022 * severity` (floor `0.20`). Each `FireSuppressionTower` within `SUPPRESSION_RADIUS = 10.5u` contributes `0.34 s^-1` cryo-foam suppression (plus `0.52 s^-1` from an arrived `RepairBot`):

```text
d(severity)/dt = -sum_towers(0.34)          if suppressed_by_tower
               = +0.028                     if unsuppressed (clamped to [0, 1])
```

Disasters clear once `severity <= 0.02`.

---

## 4. Non-Uniform Scale Normal Transform & Specular/Fresnel Lighting (`shaders/iso_factory.wgsl`, `src/iso_renderer.rs`)

For an instanced box scaled by non-uniform vector `s = (s_x, s_y, s_z)`, transforming normals by the inverse-transpose diagonal preserves surface orthogonality:

```text
n_world = normalize(n_local / max(|s|, 1e-4))
```

Surface radiance combines hemisphere sky fill, Lambertian solar diffuse (`l = normalize(sun_dir)`), Blinn-Phong specular (`h = normalize(l + v)`), and Schlick-style Fresnel rim highlighting:

```text
hemi    = 0.42 + 0.18 * n_y
ndotl   = max(dot(n, l), 0)
spec    = max(dot(n, h), 0)^28 * 0.24
fresnel = (1 - max(dot(n, v), 0))^3 * 0.16
L_out   = C_albedo * (hemi + 0.62 * ndotl) + (spec + fresnel)
```

---

## 5. License

Copyright (C) 2026 CRV Science Network. Released under **GPL-3.0-or-later** (`LICENSE`).
