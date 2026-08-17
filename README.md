# Motorbike Engine Simulator

An editable Rust vertical slice for a real-time motorbike engine simulation. It combines a
fixed-step rotational model, procedural exhaust audio, and a native diagnostic dashboard.

## Run

```sh
cargo run
```

Hold **Starter**, switch on **Ignition**, then adjust throttle. The clutch starts
connected; hold **Space** to pull it open, select one of six gears, then release Space to reconnect
the drivetrain. This makes a stationary high-gear clutch release lug or stall the engine instead of
leaving it silently disconnected. The selected gear couples crank speed to wheel speed through
clutch slip, vehicle inertia and road drag. Audio uses the Mac's default output device. The
simulator remains usable if audio cannot start.

The Up/Down arrows adjust throttle in 5% steps. Wheel load is no longer a user-controlled dyno:
the solver uses a fixed tarmac model with a 110 kg rear-axle normal load and tyre rolling
resistance, plus speed-dependent aerodynamic drag.

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

## Edit an engine

Start with [`assets/engines/inline_four_650.toml`](assets/engines/inline_four_650.toml).
The embedded profile is validated at startup; invalid physical ranges fail clearly rather than
silently producing unstable output.

This is a physically grounded educational model, not an engineering certification tool. See
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for boundaries and next stages.
See [`docs/IMPROVEMENTS.md`](docs/IMPROVEMENTS.md) for the prioritised backlog.
