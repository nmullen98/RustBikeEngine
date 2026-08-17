# Realism backlog

This is deliberately staged. Each item should add a measurable behaviour and a regression test
before the next layer is started.

## Completed foundation

- Fixed-step 1 kHz rotational solver with separate crank and vehicle/wheel inertia.
- Validated editable displacement and gear-ratio profiles.
- 720-degree four-stroke cycle with cylinder firing offsets and a 12,000 rpm redline.
- Clutch capacity/slip, gear-dependent engine braking, rolling resistance and aerodynamic drag.
- Gear/load mismatch stall guard: a fully coupled tall-gear engine now stalls below a calibrated
  low-rpm threshold instead of an idle controller keeping it artificially alive.
- Procedural audio tied to combustion, overrun, intake/exhaust resonance and output gearing.
- Delayed throttle position and intake-manifold pressure with finite filling response.
- Native GUI, keyboard controls, daily logs and crash reports.

## Current iteration

1. Add a short RPM/torque/wheel-speed history plot to make transients measurable in the lab.

## Bike motion pass — implemented

1. Keep the motorcycle fixed at the centre of the frame.
2. Scroll the tarmac only when simulated road speed is non-zero.
3. Tie wheel spoke rotation to distance travelled and wheel radius.
4. Add speed-linked suspension bob and engine-braking pitch movement.
5. Add moving lane dashes and short road-surface texture marks.
6. Add tyre contact shadows to anchor both wheels to the ground.
7. Animate visible chain links between the rear sprocket and engine drive.
8. Draw rider, frame, forks, engine block and handlebars as separate components.
9. Add headlight and engine-braking tail-light behaviour.
10. Add throttle/speed-dependent exhaust plume and a live gear overlay.

### Clutch and stall physics note

The clutch-open shift path is intentionally non-stalling: pulling the motorcycle clutch lever
disconnects the crank from the gearbox, so the engine can idle while the rider selects multiple
gears. The stall condition is clutch release with a large crank/wheel speed mismatch, especially
in a tall gear at low road speed. This is now covered by a gear-by-gear regression test.

## Physics next

3. Replace the sinusoidal power pulse with per-cylinder pressure traces using compression ratio,
   bore/stroke geometry, ignition timing and volumetric efficiency.
4. Add temperature states: coolant and oil warm-up, temperature-dependent friction, and heat from
   combustion, clutch slip and the exhaust.
5. Add clutch thermal capacity, shift interruption and a small shift-time model; report clutch
   temperature and abuse rather than silently allowing unlimited slip.
6. Add tyre grip, longitudinal load transfer, rear-wheel slip and front/rear braking torque.
7. Add road grade, wind speed and rider mass as explicit vehicle inputs.
8. Add calibrated torque/BSFC curves and a profile import format with provenance and units.

## Audio next

- Add throttle-blip, clutch-slip and gear-shift transients driven by the same state events.
- Add chain lash, tyre and wind layers tied to wheel speed, plus a mute/volume control.
- Add offline WAV rendering and a spectral comparison test against a legally sourced reference.

## Lab and engineering next

- Add live plots, cursor readouts, profile save-as, undo and deterministic replay/export.
- Add a component inspector for piston, crank, clutch, gearbox and final drive loads.
- Add SI/imperial display toggles without changing internal SI calculations.
- Add property-based finite/stability tests, dependency auditing and a locked release CI build.

The simulator remains an educational lumped model until measured pressure, torque, temperature and
audio data are supplied. UI polish should not be used to imply engineering certification.
