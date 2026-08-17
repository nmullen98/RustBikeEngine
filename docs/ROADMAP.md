# Delivery roadmap

## Phase 1 — runnable foundation (current)

- Native Rust dashboard
- Configurable 650 cc inline-four profile
- Fixed-step crankshaft dynamics
- Starter, ignition, throttle and fixed tarmac load
- Procedural, RPM-synchronised sound
- Validation and automated physics tests

## Phase 2 — credible four-stroke cycle

- 720-degree cycle and explicit firing order
- Piston position, velocity and acceleration
- Per-cylinder pressure traces
- Intake/exhaust valve timing
- Live torque ripple plot

Exit condition: conservation and cycle tests pass, and simulated full-load torque is calibrated
against a cited reference curve.

## Phase 3 — drivetrain and motorcycle components

- Wet clutch and clutch slip
- Sequential gearbox and shift interruption
- Chain/final-drive ratio
- Rear-wheel inertia, aerodynamic drag and rolling resistance
- Component inspector with force/temperature limits

## Phase 4 — sound calibration

- Intake and exhaust resonator networks
- Even 180-degree four-cylinder firing-order timing
- Load-sensitive combustion texture
- Mechanical whine and transmission lash
- Offline WAV export and spectrum comparison

## Required reference resources

- Manufacturer bore, stroke, compression, firing-order and redline specifications
- Rear-wheel or crank dynamometer data with test conditions
- Valve timing and lift data, where legally available
- Clean recordings at known RPM/load and microphone position
- Gear ratios, primary reduction, final drive, wheel/tyre dimensions and vehicle mass

Keep source licences and measurement conditions beside every imported dataset.
