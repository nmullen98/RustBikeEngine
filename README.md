# Motorbike Engine Simulator

An editable Rust lab for a real-time motorbike engine simulation. It combines a fixed-step
rotational model, procedural exhaust audio, and a native diagnostic dashboard.

This is an educational, deterministic lumped model—not an engineering certification tool.

## Features

- 1 kHz four-stroke crank, clutch, gearbox, tyre-load and road model.
- Editable inline-four TOML profile with strict finite-range validation.
- Traction-limited rear-wheel torque, engine braking, stalling, rolling resistance and aero drag.
- Native `eframe` dashboard, keyboard controls, local diagnostics, and procedural CPAL audio.
- Regression tests for starting, redline, stalling, transient intake response, tyre force and travel.

## Requirements

- Rust `1.97` or later (the pinned toolchain is selected automatically by `rust-toolchain.toml`).
- A desktop environment supported by `eframe`; audio is optional and the lab runs without it.

## Run

```sh
cargo run
```

For an optimised build:

```sh
cargo run --release
```

Hold **Starter**, switch on **Ignition**, then adjust throttle. The clutch starts
connected; hold **Space** to pull it open, select one of six gears, then release Space to reconnect
the drivetrain. This makes a stationary high-gear clutch release lug or stall the engine instead of
leaving it silently disconnected. The selected gear couples crank speed to wheel speed through
clutch slip, vehicle inertia and road drag. Audio uses the Mac's default output device. The
simulator remains usable if audio cannot start.

The Up/Down arrows adjust throttle in 5% steps. Wheel load is no longer a user-controlled dyno:
the solver uses a fixed tarmac model with a 110 kg rear-axle normal load, rolling resistance,
speed-dependent aerodynamic drag and a dry-tarmac tyre-grip cap. The wheel-torque readout shows
the torque actually transmitted to the ground and warns when tyre grip limits the request.

## Controls

| Control | Action |
| --- | --- |
| Ignition checkbox | Enable or disable combustion and idle control. |
| Hold Starter | Apply starter-motor torque. |
| Up / Down | Increase / decrease throttle by 5%. |
| Space | Hold to disengage the clutch. |
| Left / Right | Shift down / up while the clutch is open. |
| Pause | Pause the fixed-step physics loop. |

The solver models a 720-degree four-stroke sequence (intake, compression, power and exhaust)
using the configured firing offsets. Intake manifold pressure now follows throttle position with
short physical lags, so throttle blips and lift-off do not change torque instantaneously. Sound is
synthesised from the same firing layout. The inline four uses even 180° firing intervals,
separate intake and exhaust resonances, load-sensitive combustion pulses, mechanical harmonics
and restrained fuel-cut overrun pops.

Open **Engine and gearbox setup** to change displacement and individual gear ratios. Review the
estimated peak torque, then select **Apply to simulation**. Invalid or non-descending ratios are
rejected without changing the running model. Displacement scales torque, pumping loss and rotating
inertia relative to the reference engine. GUI edits last for the current session; update the TOML
profile to make them the next startup defaults.

## Crash logs

Open **Diagnostics** in the left panel to see the exact log directory. The application writes:

- `simulator.log.YYYY-MM-DD` — startup, engine state and audio/device events.
- `crash.log` — synchronous panic details and a backtrace, preserved if the app crashes.

On macOS these normally live under `~/Library/Application Support/` in the simulator's `logs`
folder. Logs contain technical state only; engine control values are not continuously recorded.

## Quality checks

```sh
cargo fmt --check
cargo quality
cargo verify
```

`cargo quality` runs strict Clippy. `cargo verify` runs all unit and integration tests.

## Edit an engine

Start with [`assets/engines/inline_four_650.toml`](assets/engines/inline_four_650.toml).
The embedded profile is validated at startup; invalid physical ranges fail clearly rather than
silently producing unstable output.

## Documentation

- [`docs/CODE_GUIDE.md`](docs/CODE_GUIDE.md) — source-module responsibilities and runtime flow.
- [`docs/CONFIGURATION.md`](docs/CONFIGURATION.md) — every editable TOML parameter and unit.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — boundaries, accuracy model, and next stages.
- [`docs/PHYSICS_REVIEW.md`](docs/PHYSICS_REVIEW.md) — reviewed assumptions and known limits.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — development and change rules.
- [`SECURITY.md`](SECURITY.md) — security scope and vulnerability reporting.

See [`docs/IMPROVEMENTS.md`](docs/IMPROVEMENTS.md) for the prioritised backlog.
