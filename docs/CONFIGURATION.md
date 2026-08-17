# Engine-profile configuration

The bundled startup profile is [`assets/engines/inline_four_650.toml`](../assets/engines/inline_four_650.toml).
Profiles are embedded at compile time; edit the TOML and rebuild to change startup defaults. The
setup panel can make temporary edits but does not write the profile file.

All values must be finite and within bounds enforced in `src/config.rs`. Invalid profiles are
rejected before they reach the solver.

## Engine fields

| Field | Unit | Meaning |
| --- | --- | --- |
| `name` | text | Dashboard/profile name. |
| `layout` | text | Firing-layout identifier; `inline_four` uses even firing spacing and `parallel_twin_270` uses 270° spacing. |
| `cylinders` | count | Cylinder count, currently 1–8. |
| `cycle_strokes` | count | Must be `4`. |
| `displacement_cc` | cc | Active swept volume; scales torque, pumping loss and rotating inertia. |
| `reference_displacement_cc` | cc | Calibration displacement used as the scale denominator. |
| `bore_mm`, `stroke_mm` | mm | Engine geometry metadata retained for future cylinder-pressure work. |
| `compression_ratio` | ratio | Geometry metadata retained for future combustion work. |
| `idle_rpm`, `redline_rpm` | rpm | Idle target and hard speed limit. |
| `flywheel_inertia_kg_m2`, `rotating_inertia_kg_m2` | kg·m² | Crank/rotating inertia. |
| `max_torque_nm`, `peak_torque_rpm` | N·m, rpm | Mean peak torque calibration and curve centre. |
| `friction_nm_at_idle`, `friction_nm_at_redline` | N·m | Mechanical friction endpoints. |
| `max_pumping_brake_nm` | N·m | Maximum closed-throttle pumping loss. |
| `starter_torque_nm` | N·m | Starter torque below the starter cutoff. |
| `idle_base_throttle`, `idle_control_gain` | 0–1 | Base airflow and proportional idle correction. |
| `ambient_pressure_kpa`, `idle_manifold_pressure_kpa` | kPa absolute | Intake pressure endpoints. |
| `throttle_response_seconds`, `manifold_fill_seconds` | s | First-order intake lag time constants. |
| `exhaust_primary_hz`, `exhaust_secondary_hz`, `intake_resonance_hz` | Hz | Procedural-audio resonance calibrations. |

## `[gearbox]` fields

| Field | Unit | Meaning |
| --- | --- | --- |
| `primary_reduction` | ratio | Crankshaft-to-gearbox-input reduction. |
| `gear_ratios` | ratio list | First-to-top forward ratios; must strictly decrease. |
| `final_drive_ratio` | ratio | Front-to-rear sprocket reduction. |
| `transmission_efficiency` | 0–1 | Drivetrain torque fraction after losses. |
| `rear_wheel_radius_m` | m | Loaded rear-wheel radius. |
| `rear_axle_load_kg` | kg | Static normal load used for rolling resistance and grip. |
| `tyre_rolling_resistance_coefficient` | coefficient | Rolling-resistance factor. |
| `tyre_peak_friction_coefficient` | coefficient | Static dry-surface longitudinal grip factor. |
| `vehicle_mass_kg` | kg | Total mass reflected to the wheel axis. |
| `wheel_inertia_kg_m2` | kg·m² | Physical rear-wheel rotational inertia. |
| `aero_drag_nm_at_100_kph` | N·m | Wheel-axis aerodynamic drag calibration at 100 km/h. |
| `clutch_capacity_nm` | N·m | Maximum transferable clutch torque. |
| `clutch_stiffness_nm_per_rad_s` | N·m/(rad/s) | Torque response to clutch slip speed before capacity limiting. |

The current tyre model caps contact-patch torque using static axle load. It does not model wheelspin,
front/rear load transfer, braking, or changing road surfaces.
