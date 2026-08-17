# Physics review — 650 cc inline-four lab

## Verdict

The lab is a sound educational lumped-parameter simulator: it has a fixed 1 kHz update, bounded
inputs, a four-stroke firing sequence, crank inertia, clutch slip, ratios, vehicle-equivalent
wheel inertia, rolling resistance and aerodynamic drag. It is not yet a predictive motorcycle
model. Several parts are intentionally calibrated heuristics rather than measured engine or tyre
physics.

This review separates what is causal and useful from what currently limits realism. The three
implemented corrections affect every acceleration, shift and visual result: mean engine torque
calibration, available rear-tyre force, and travelled distance.

## Current model in plain language

### Engine and intake

Throttle plate position follows the rider command through a first-order lag. Manifold pressure
then follows a separate first-order fill response. That pressure becomes a normalised air-charge
proxy. The proxy feeds a calibrated RPM torque curve, a non-linear throttle demand and a
four-stroke pulse factor. Mechanical friction and manifold pumping loss oppose combustion.

This captures delayed throttle response, fuel-cut overrun and a recognisable idle. It does not
calculate cylinder mass, fuel mass, spark timing, residual gas, volumetric efficiency or a real
pressure trace.

### Crank, clutch and gears

Net crank torque is integrated through a combined flywheel/rotating inertia. Clutch torque is a
slip-speed spring limited by clutch capacity. The selected primary, gearbox and final-drive ratios
multiply that torque to the rear wheel. With the clutch pulled, the engine and wheel are isolated;
with it released, low engine speed in a tall gear can stall the engine.

The solver correctly models the direction of clutch torque and engine braking, but it has no
shift duration, dog engagement model, clutch temperature or separate gearbox shaft inertias.

### Vehicle and road

Vehicle mass is reflected to the rear-wheel axis as `m × r²`; this combines translational vehicle
inertia with wheel inertia in one rotational equation. Rolling resistance is calculated from the
configured rear-axle normal load, tyre rolling coefficient and wheel radius. Aerodynamic drag is
quadratic in speed and applied at the wheel.

Before this review there was no cap on tyre force. That allowed drivetrain torque above what a
110 kg rear axle on dry tarmac could transmit. There was also no integrated travelled distance;
the visual road used current speed multiplied by wall-clock time.

## Findings and priority

| Priority | Finding | Why it matters | Resolution |
| --- | --- | --- | --- |
| P0 | The four-stroke pulse averaged about 0.596, so a configured `64 Nm` engine produced roughly 60% of its calibrated mean torque before other losses. | Torque, acceleration and shift behaviour were miscalibrated everywhere. | Resolved: pulse mean is 1.0 with bounded torque ripple. |
| P0 | Rear-wheel torque was unlimited by tyre friction. | First-gear acceleration and wheel load could exceed the available tarmac force. | Resolved: cap drive/braking torque using `normal load × tyre friction × wheel radius`; expose when the cap is active. |
| P0 | Travel distance was not integrated by the physics solver. | Wheel rotation and road movement were display estimates, not actual model state. | Resolved: integrate distance from wheel speed and expose it through gearbox state. |
| P1 | The stall rule was a fixed 35% idle-RPM threshold. | It could not distinguish a brief torque-pulse dip from sustained clutch overload, and had no restart state. | Resolved first stage: clutch load below a combustion-stability speed accumulates time exposure; a stall latches until starter/bump-start recovery. A measured stall map remains future work. |
| P1 | Tyre grip is a torque cap, not a tyre-slip model. | The cap prevents impossible force but cannot model burnouts, ABS, wheelspin or load transfer. | Add separate wheel and vehicle speeds, slip ratio, longitudinal tyre curve and dynamic axle load. |
| P1 | Combustion is a calibrated torque curve with a sinusoidal power window. | The model cannot predict changes from compression ratio, cam timing, ignition or exhaust tuning. | Add per-cylinder cylinder volume, trapped mass, pressure trace and ignition timing. |
| P2 | No oil/coolant/clutch temperature states. | Repeated pulls and clutch abuse never change friction, torque or reliability. | Add heat capacity, heat rejection and temperature-dependent friction. |
| P2 | No road grade, headwind or rider mass control. | Hill starts and real-world top-speed/load comparisons are missing. | Add grade, wind and mass inputs with deterministic replay. |
| P2 | No shift-time or driveline lash. | Gear changes are instant and quiet in the physics layer. | Add shift cut, dog engagement delay, chain compliance and gearbox shaft inertias. |

## Selected corrections and acceptance criteria

### 1. Mean torque calibration

The previous pulse factor used a constant floor plus an average per-cylinder sine pulse. For a
180-degree sine-shaped power stroke in a 720-degree cycle, the mean per-cylinder pulse is
`1 / (2π)`, approximately `0.159`. The old factor therefore averaged
`0.52 + 0.48 × 0.159 = 0.596`. This makes the value called `max_torque_nm` misleading.

The correction normalises the summed firing pulses to their analytical cycle mean, then blends the
normalised result with a bounded ripple floor. The average factor is exactly 1.0, so the torque
profile remains calibrated in the mean while the flywheel still smooths visible firing events.

Acceptance: a sampled full 720-degree cycle averages `1.0 ± 0.01` for the bundled inline four.

### 2. Available tyre force

The starting approximation uses a dry-tarmac peak friction coefficient and the configured static
rear-axle load. Maximum contact-patch force is `normal load × friction coefficient`; wheel torque
is that force multiplied by tyre radius. For the bundled `110 kg`, `1.05` and `0.315 m` values,
the cap is approximately `357 Nm`.

This is deliberately not a complete tyre model. Torque beyond the cap is reported as traction
limited rather than treated as extra vehicle acceleration. Wheelspin energy is not yet conserved;
the next tyre-slip stage must add a separately rotating rear wheel.

Acceptance: applied rear-wheel torque never exceeds the configured grip torque, and the state
flags when the cap is active.

### 3. Integrated travel distance

Distance is integrated each fixed physics step from rear-wheel angular speed and tyre radius. The
GUI uses that state for ground scroll and spoke/chain motion, so pausing physics freezes the world
and distance changes only when the vehicle model moves.

Acceptance: distance is monotonic while moving, unchanged while stationary, and is available in
the observable gearbox state.

## Deferred changes

Do not add more sliders before the P1 tyre-slip and cylinder-pressure work has reference data.
Extra controls without measured calibration would make the lab look more precise while making the
results less trustworthy.
