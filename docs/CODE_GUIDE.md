# Code guide

## Runtime path

```text
TOML profile ──validate──> EngineSimulation ──snapshot──> egui dashboard
                                  │                    └─> bike animation
                                  └─atomic snapshot───> CPAL audio callback
```

`src/lib.rs` exposes the documented library modules and denies undocumented public items.
`src/main.rs` starts logging, installs the panic hook, creates the native window, and owns the
application lifetime.

## Source modules

| Path | Responsibility | Important rule |
| --- | --- | --- |
| `src/config.rs` | TOML deserialisation, profile validation, derived reduction/inertia/load values. | Treat profiles as data only; reject non-finite and out-of-range values before simulation. |
| `src/simulation.rs` | Deterministic 1 ms engine, intake, clutch, drivetrain, tyre-cap and travel-distance solver. | Keep SI units internally and add a regression test with every model change. |
| `src/audio.rs` | Allocation-free CPAL callback and procedural firing/exhaust/intake/transmission sound. | The callback must not lock, allocate, log, open files, or perform I/O. |
| `src/app.rs` | `egui` controls, fixed-step accumulator, setup editor, diagnostics and rendering. | Presentation must consume state; it must not change physics outside explicit inputs. |
| `src/logging.rs` | Daily local logs and synchronous crash reports. | Log technical diagnostics only; never add credentials or continuous user telemetry. |
| `src/main.rs` | Native application startup and shutdown policy. | Keep startup wiring small and keep physics in the library modules. |
| `tests/engine_behaviour.rs` | Black-box behavioural regression tests. | Tests should express observable physical outcomes rather than private implementation details. |

## Simulation order

Each accepted time step follows this sequence:

1. Clamp rider input and calculate idle assistance/fuel-cut state.
2. Integrate throttle-plate and manifold-pressure first-order lags.
3. Calculate calibrated combustion, friction, pumping, starter and clutch torque.
4. Integrate crank angular velocity through crank inertia.
5. Transform clutch torque through the selected ratios; cap transmitted tyre torque at static grip.
6. Integrate rear-wheel/vehicle-equivalent inertia against rolling and aerodynamic resistance.
7. Integrate distance from rear-wheel speed and publish engine/gearbox snapshots.

`docs/PHYSICS_REVIEW.md` records which parts are calibrated approximations. In particular, the
static tyre-force cap is not a tyre-slip or load-transfer model, and power pulses are not measured
cylinder-pressure traces.

## Public library use

```rust
use motorbike_engine_sim::{
    config::EngineConfig,
    simulation::{EngineInputs, EngineSimulation},
};

let config = EngineConfig::load_default()?;
let mut simulation = EngineSimulation::new(config);
simulation.set_inputs(EngineInputs { starter: true, ..EngineInputs::default() });
for _ in 0..1_000 {
    simulation.step(0.001);
}
let state = simulation.state();
# Ok::<(), Box<dyn std::error::Error>>(())
```

The public API is documented with Rustdoc. Generate and inspect it locally with:

```sh
cargo doc --no-deps --open
```
