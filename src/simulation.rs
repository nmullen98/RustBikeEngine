//! Deterministic fixed-step four-stroke engine, clutch, drivetrain, and road model.
//!
//! All internal calculations use SI units. Call [`EngineSimulation::step`] with
//! a fixed 1 ms time step for the calibrated behaviour described by the profile.

use crate::config::{ConfigError, EngineConfig};
use std::f64::consts::TAU;

const RPM_PER_RADIAN_PER_SECOND: f64 = 60.0 / TAU;
const COMBUSTION_CUTOFF_RPM: f64 = 280.0;
const COMBUSTION_FULL_RPM: f64 = 650.0;
const STALL_EXPOSURE_SECONDS: f64 = 0.12;
const AMBIENT_TEMPERATURE_C: f64 = 20.0;
const SHIFT_CUT_SECONDS: f64 = 0.06;
const OVERHEAT_TEMPERATURE_C: f64 = 110.0;
const FOUR_STROKE_CYCLE_RADIANS: f64 = TAU * 2.0;
const POWER_STROKE_RADIANS: f64 = TAU * 0.5;

/// The four phases of a cylinder's 720-degree four-stroke cycle.
///
/// The dashboard reports cylinder one's phase. Other cylinders use their own
/// firing offset from the selected firing layout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FourStroke {
    /// Induction phase: piston travels down and cylinder charge is admitted.
    #[default]
    Intake,
    /// Compression phase: piston travels up before ignition.
    Compression,
    /// Expansion phase: combustion pressure produces positive crank torque.
    Power,
    /// Exhaust phase: piston expels spent gas.
    Exhaust,
}

impl FourStroke {
    #[must_use]
    /// Returns a stable human-readable phase name for the dashboard.
    pub fn label(self) -> &'static str {
        match self {
            Self::Intake => "Intake",
            Self::Compression => "Compression",
            Self::Power => "Power",
            Self::Exhaust => "Exhaust",
        }
    }

    #[must_use]
    /// Maps a 720-degree crank-cycle angle to cylinder one's four-stroke phase.
    pub fn from_cycle_angle(cycle_angle_rad: f64) -> Self {
        let angle = cycle_angle_rad.rem_euclid(FOUR_STROKE_CYCLE_RADIANS);
        match angle {
            angle if angle < TAU * 0.5 => Self::Intake,
            angle if angle < TAU => Self::Compression,
            angle if angle < TAU * 1.5 => Self::Power,
            _ => Self::Exhaust,
        }
    }
}

/// User-controlled inputs. Values are clamped by the solver.
#[derive(Debug, Clone, Copy)]
pub struct EngineInputs {
    /// Whether ignition enables combustion and idle control.
    pub ignition: bool,
    /// Whether the starter contributes cranking torque below its cutoff speed.
    pub starter: bool,
    /// Rider throttle request, clamped to the inclusive range `0.0..=1.0`.
    pub throttle: f64,
    /// Selected forward gear; zero is neutral and values above top gear are clamped.
    pub gear: u8,
    /// Zero is disengaged; one is fully engaged.
    pub clutch_engagement: f64,
}

impl Default for EngineInputs {
    fn default() -> Self {
        Self {
            ignition: true,
            starter: false,
            throttle: 0.0,
            gear: 0,
            // The clutch is connected by default; Space temporarily opens it
            // for a shift, matching a real motorcycle clutch lever.
            clutch_engagement: 1.0,
        }
    }
}

/// Observable engine state. SI units are used internally.
#[derive(Debug, Clone, Copy, Default)]
pub struct EngineState {
    /// Crankshaft speed in revolutions per minute.
    pub rpm: f64,
    /// Crank angle modulo 360 degrees, in radians.
    pub crank_angle_rad: f64,
    /// Absolute phase within the 720-degree four-stroke cycle.
    pub cycle_angle_rad: f64,
    /// Current phase for cylinder one's cycle.
    pub stroke: FourStroke,
    /// Delayed throttle plate position, from zero to one.
    pub throttle_position: f64,
    /// Simplified intake manifold absolute pressure.
    pub manifold_pressure_kpa: f64,
    /// Instantaneous positive torque from the calibrated combustion model, in N·m.
    pub combustion_torque_nm: f64,
    /// Mechanical friction torque opposing rotation, in N·m.
    pub friction_torque_nm: f64,
    /// Closed-throttle pumping torque opposing rotation, in N·m.
    pub pumping_torque_nm: f64,
    /// Sum of friction and pumping torque, in N·m.
    pub engine_braking_torque_nm: f64,
    /// Torque transferred from the crank to clutch/drivetrain, in N·m.
    pub clutch_torque_nm: f64,
    /// Net torque used to accelerate the crank, in N·m.
    pub net_torque_nm: f64,
    /// Air-charge proxy after intake lag and idle control, from zero to one.
    pub effective_throttle: f64,
    /// Whether clutch load reduced the engine below sustainable combustion speed.
    ///
    /// A stalled engine remains off until it is cranked or bump-started above
    /// the combustion relight speed.
    pub stalled: bool,
    /// Coolant temperature in degrees Celsius.
    pub coolant_temperature_c: f64,
    /// Oil temperature in degrees Celsius.
    pub oil_temperature_c: f64,
    /// Clutch pack temperature in degrees Celsius.
    pub clutch_temperature_c: f64,
    /// Exhaust temperature in degrees Celsius.
    pub exhaust_temperature_c: f64,
    /// Whether the engine is currently losing power to thermal protection.
    pub overheating: bool,
    /// Whether the ignition is briefly cut during a gear change.
    pub shift_cut_active: bool,
    /// Torque shock generated by a clutchless shift, in N·m.
    pub shift_shock_nm: f64,
}

/// Derived gearbox and rear-wheel measurements for the current engine state.
#[derive(Debug, Clone, Copy, Default)]
pub struct GearboxState {
    /// Active forward gear; zero represents neutral.
    pub selected_gear: u8,
    /// Combined primary, selected-gear, and final-drive ratio.
    pub overall_ratio: f64,
    /// Rear-wheel rotational speed, in revolutions per minute.
    pub output_rpm: f64,
    /// Torque requested at the rear wheel before the tyre-force cap.
    pub requested_rear_wheel_torque_nm: f64,
    /// Actual torque transferred through the rear contact patch.
    pub rear_wheel_torque_nm: f64,
    /// Whether requested torque exceeded the static tyre-force cap this step.
    pub traction_limited: bool,
    /// Vehicle speed derived from rear-wheel angular velocity, in km/h.
    pub road_speed_kph: f64,
    /// Crank speed minus wheel speed reflected through the selected ratio, in rpm.
    pub clutch_slip_rpm: f64,
    /// Non-decreasing longitudinal distance integrated from rear-wheel speed, in metres.
    pub distance_m: f64,
    /// Whether a gear change is currently in its ignition-cut interval.
    pub shift_in_progress: bool,
    /// Whether the last gear change was made with the clutch engaged.
    pub clutchless_shift: bool,
}

impl EngineState {
    #[must_use]
    /// Returns whether engine speed has reached stable-combustion speed.
    pub fn is_running(self) -> bool {
        self.rpm >= COMBUSTION_FULL_RPM
    }
}

/// Fixed-step rotational engine model.
///
/// The model owns validated calibration, inputs, and the evolving engine and
/// road state. It is deterministic for an identical configuration, input
/// sequence, and time-step sequence. It intentionally does not model tyre
/// slip, thermal state, emissions, or measured cylinder pressure.
pub struct EngineSimulation {
    config: EngineConfig,
    state: EngineState,
    inputs: EngineInputs,
    throttle_position: f64,
    manifold_pressure_kpa: f64,
    wheel_angular_velocity_rad_s: f64,
    requested_rear_wheel_torque_nm: f64,
    rear_wheel_torque_nm: f64,
    traction_limited: bool,
    distance_m: f64,
    stall_exposure_seconds: f64,
    stalled: bool,
    coolant_temperature_c: f64,
    oil_temperature_c: f64,
    clutch_temperature_c: f64,
    exhaust_temperature_c: f64,
    shift_timer_seconds: f64,
    shift_shock_nm: f64,
    clutchless_shift: bool,
}

impl EngineSimulation {
    #[must_use]
    /// Creates a stationary simulation using a previously validated profile.
    pub fn new(config: EngineConfig) -> Self {
        let ambient_pressure_kpa = config.ambient_pressure_kpa;
        Self {
            config,
            state: EngineState {
                manifold_pressure_kpa: ambient_pressure_kpa,
                ..EngineState::default()
            },
            inputs: EngineInputs::default(),
            throttle_position: 0.0,
            manifold_pressure_kpa: ambient_pressure_kpa,
            wheel_angular_velocity_rad_s: 0.0,
            requested_rear_wheel_torque_nm: 0.0,
            rear_wheel_torque_nm: 0.0,
            traction_limited: false,
            distance_m: 0.0,
            stall_exposure_seconds: 0.0,
            stalled: false,
            coolant_temperature_c: AMBIENT_TEMPERATURE_C,
            oil_temperature_c: AMBIENT_TEMPERATURE_C,
            clutch_temperature_c: AMBIENT_TEMPERATURE_C,
            exhaust_temperature_c: AMBIENT_TEMPERATURE_C,
            shift_timer_seconds: 0.0,
            shift_shock_nm: 0.0,
            clutchless_shift: false,
        }
    }

    #[must_use]
    /// Returns the active, validated configuration.
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    #[must_use]
    /// Returns a copy of the latest engine-state snapshot.
    pub fn state(&self) -> EngineState {
        self.state
    }

    #[must_use]
    /// Returns the clamped rider input state currently used by the solver.
    pub fn inputs(&self) -> EngineInputs {
        self.inputs
    }

    /// Replaces inputs after clamping throttle, gear, and clutch engagement to safe ranges.
    pub fn set_inputs(&mut self, mut inputs: EngineInputs) {
        inputs.throttle = inputs.throttle.clamp(0.0, 1.0);
        inputs.gear = inputs.gear.min(self.config.gearbox.forward_gears());
        inputs.clutch_engagement = inputs.clutch_engagement.clamp(0.0, 1.0);
        if inputs.gear != self.inputs.gear {
            self.shift_timer_seconds = SHIFT_CUT_SECONDS;
            self.clutchless_shift = inputs.clutch_engagement > 0.2;
        }
        self.inputs = inputs;
    }

    /// Applies a validated calibration without discarding the running state.
    ///
    /// # Errors
    ///
    /// Returns the first invalid engine or gearbox value and leaves the active setup unchanged.
    pub fn update_config(&mut self, config: EngineConfig) -> Result<(), ConfigError> {
        config.validate()?;
        self.config = config;
        self.state.rpm = self.state.rpm.min(self.config.redline_rpm);
        self.manifold_pressure_kpa = self
            .manifold_pressure_kpa
            .clamp(0.0, self.config.ambient_pressure_kpa);
        self.state.manifold_pressure_kpa = self.manifold_pressure_kpa;
        self.set_inputs(self.inputs);
        Ok(())
    }

    #[must_use]
    /// Returns derived transmission and vehicle measurements from the latest step.
    pub fn gearbox_state(&self) -> GearboxState {
        let wheel_rpm = self.wheel_angular_velocity_rad_s * RPM_PER_RADIAN_PER_SECOND;
        let Some(ratio) = self.config.gearbox.overall_ratio(self.inputs.gear) else {
            return GearboxState {
                road_speed_kph: wheel_rpm * TAU * self.config.gearbox.rear_wheel_radius_m * 60.0
                    / 1000.0,
                distance_m: self.distance_m,
                ..GearboxState::default()
            };
        };
        let output_rpm = wheel_rpm;
        let road_speed_kph =
            wheel_rpm * TAU * self.config.gearbox.rear_wheel_radius_m * 60.0 / 1000.0;
        let clutch_slip_rpm = self.state.rpm - wheel_rpm * ratio;
        GearboxState {
            selected_gear: self.inputs.gear,
            overall_ratio: ratio,
            output_rpm,
            requested_rear_wheel_torque_nm: self.requested_rear_wheel_torque_nm,
            rear_wheel_torque_nm: self.rear_wheel_torque_nm,
            traction_limited: self.traction_limited,
            road_speed_kph,
            clutch_slip_rpm,
            distance_m: self.distance_m,
            shift_in_progress: self.shift_timer_seconds > 0.0,
            clutchless_shift: self.clutchless_shift,
        }
    }

    /// Advances the model by a positive time step up to 20 ms.
    ///
    /// Use a fixed 1 ms `dt_seconds` for the intended calibration. Invalid,
    /// zero, and over-large time steps are ignored rather than destabilising the solver.
    #[allow(clippy::too_many_lines)]
    pub fn step(&mut self, dt_seconds: f64) {
        if !(0.0..=0.02).contains(&dt_seconds) || dt_seconds == 0.0 {
            return;
        }
        let rpm = self.state.rpm;
        let idle_error = ((self.config.idle_rpm - rpm) / self.config.idle_rpm).max(0.0);
        let idle_throttle = if self.inputs.ignition && rpm < self.config.idle_rpm * 1.35 {
            self.config.idle_base_throttle + self.config.idle_control_gain * idle_error
        } else {
            0.0
        };
        let overrun_fuel_cut = self.inputs.ignition
            && self.inputs.throttle <= 0.01
            && rpm > self.config.idle_rpm * 1.5;
        let (effective_throttle, manifold_air_fraction) =
            self.update_intake_air(dt_seconds, rpm, idle_throttle, overrun_fuel_cut);

        let shift_cut_active = self.shift_timer_seconds > 0.0;
        let combustion_enabled = self.inputs.ignition
            && !overrun_fuel_cut
            && !shift_cut_active
            && (!self.stalled || self.inputs.starter || rpm >= COMBUSTION_FULL_RPM);
        let combustion_ramp = if combustion_enabled {
            ((rpm - COMBUSTION_CUTOFF_RPM) / (COMBUSTION_FULL_RPM - COMBUSTION_CUTOFF_RPM))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        let torque_curve = self.torque_curve(rpm);
        // A butterfly throttle's useful flow area is deliberately non-linear near closed.
        let torque_demand = effective_throttle * (2.0 - effective_throttle);
        let cycle_power_factor = self.four_stroke_power_factor(self.state.cycle_angle_rad);
        let combustion_torque = self.config.effective_max_torque_nm()
            * torque_demand
            * torque_curve
            * combustion_ramp
            * cycle_power_factor
            * self.thermal_derate();
        let friction_torque = self.friction_torque(rpm);
        let pumping_torque = self.pumping_torque(rpm, manifold_air_fraction);
        let engine_braking_torque = friction_torque + pumping_torque;
        let starter_torque = if self.inputs.starter && rpm < 900.0 {
            self.config.starter_torque_nm * (1.0 - rpm / 1100.0).max(0.15)
        } else {
            0.0
        };
        let crank_angular_speed = rpm / RPM_PER_RADIAN_PER_SECOND;
        let clutch_torque = self.clutch_torque(crank_angular_speed);
        let net_torque = combustion_torque + starter_torque - engine_braking_torque - clutch_torque;

        self.update_thermal_state(
            dt_seconds,
            combustion_torque,
            rpm,
            clutch_torque,
            crank_angular_speed - self.wheel_angular_velocity_rad_s,
        );
        self.shift_timer_seconds = (self.shift_timer_seconds - dt_seconds).max(0.0);
        self.shift_shock_nm = if shift_cut_active && self.clutchless_shift {
            (clutch_torque.abs() * 0.35).min(120.0)
        } else {
            0.0
        };

        let angular_acceleration = net_torque / self.config.total_inertia_kg_m2();
        let mut angular_speed = crank_angular_speed;
        angular_speed = (angular_speed + angular_acceleration * dt_seconds).max(0.0);
        let mut next_rpm = angular_speed * RPM_PER_RADIAN_PER_SECOND;

        let requested_wheel_torque = self.drivetrain_wheel_torque(clutch_torque);
        let maximum_tyre_torque = self.config.gearbox.max_tyre_torque_nm();
        let applied_wheel_torque =
            requested_wheel_torque.clamp(-maximum_tyre_torque, maximum_tyre_torque);
        self.requested_rear_wheel_torque_nm = requested_wheel_torque;
        self.rear_wheel_torque_nm = applied_wheel_torque;
        self.traction_limited =
            (requested_wheel_torque - applied_wheel_torque).abs() > f64::EPSILON;

        let wheel_drag =
            self.config.gearbox.static_tarmac_load_torque_nm() + self.aero_drag_torque();
        let wheel_angular_acceleration =
            (applied_wheel_torque - wheel_drag) / self.config.gearbox.wheel_inertia_kg_m2();
        self.wheel_angular_velocity_rad_s =
            (self.wheel_angular_velocity_rad_s + wheel_angular_acceleration * dt_seconds).max(0.0);
        self.distance_m += self.wheel_angular_velocity_rad_s
            * self.config.gearbox.rear_wheel_radius_m
            * dt_seconds;

        if next_rpm > self.config.redline_rpm {
            next_rpm = self.config.redline_rpm;
        }
        next_rpm = self.resolve_stall(
            dt_seconds,
            next_rpm,
            combustion_torque,
            starter_torque,
            engine_braking_torque,
            clutch_torque,
        );
        if next_rpm < 15.0 && !self.inputs.starter && combustion_torque == 0.0 {
            next_rpm = 0.0;
        }

        let next_cycle_angle = (self.state.cycle_angle_rad + angular_speed * dt_seconds)
            .rem_euclid(FOUR_STROKE_CYCLE_RADIANS);

        self.state = EngineState {
            rpm: next_rpm,
            crank_angle_rad: next_cycle_angle % TAU,
            cycle_angle_rad: next_cycle_angle,
            stroke: FourStroke::from_cycle_angle(next_cycle_angle),
            throttle_position: self.throttle_position,
            manifold_pressure_kpa: self.manifold_pressure_kpa,
            combustion_torque_nm: combustion_torque,
            friction_torque_nm: friction_torque,
            pumping_torque_nm: pumping_torque,
            engine_braking_torque_nm: engine_braking_torque,
            clutch_torque_nm: clutch_torque,
            net_torque_nm: net_torque,
            effective_throttle,
            stalled: self.stalled,
            coolant_temperature_c: self.coolant_temperature_c,
            oil_temperature_c: self.oil_temperature_c,
            clutch_temperature_c: self.clutch_temperature_c,
            exhaust_temperature_c: self.exhaust_temperature_c,
            overheating: self.coolant_temperature_c >= OVERHEAT_TEMPERATURE_C,
            shift_cut_active,
            shift_shock_nm: self.shift_shock_nm,
        };
    }

    fn thermal_derate(&self) -> f64 {
        if self.coolant_temperature_c <= OVERHEAT_TEMPERATURE_C {
            1.0
        } else {
            (1.0 - (self.coolant_temperature_c - OVERHEAT_TEMPERATURE_C) / 80.0).clamp(0.65, 1.0)
        }
    }

    fn update_thermal_state(
        &mut self,
        dt_seconds: f64,
        combustion_torque: f64,
        rpm: f64,
        clutch_torque: f64,
        clutch_slip_rad_s: f64,
    ) {
        let engine_heat = 0.75 + combustion_torque.abs() * rpm * 0.000_012;
        let coolant_cooling = (self.coolant_temperature_c - AMBIENT_TEMPERATURE_C) * 0.075;
        self.coolant_temperature_c += (engine_heat - coolant_cooling) * dt_seconds;
        self.oil_temperature_c += ((engine_heat * 0.72)
            - (self.oil_temperature_c - AMBIENT_TEMPERATURE_C) * 0.055)
            * dt_seconds;
        let clutch_heat = clutch_torque.abs() * clutch_slip_rad_s.abs() * 0.001_8;
        self.clutch_temperature_c +=
            (clutch_heat - (self.clutch_temperature_c - AMBIENT_TEMPERATURE_C) * 0.12) * dt_seconds;
        let exhaust_heat = combustion_torque.abs() * 0.018 + rpm * 0.0008;
        self.exhaust_temperature_c += (exhaust_heat
            - (self.exhaust_temperature_c - AMBIENT_TEMPERATURE_C) * 0.11)
            * dt_seconds;
    }

    fn update_intake_air(
        &mut self,
        dt_seconds: f64,
        rpm: f64,
        idle_throttle: f64,
        overrun_fuel_cut: bool,
    ) -> (f64, f64) {
        let throttle_response =
            1.0 - (-dt_seconds / self.config.throttle_response_seconds.max(f64::EPSILON)).exp();
        self.throttle_position +=
            (self.inputs.throttle - self.throttle_position) * throttle_response;
        self.throttle_position = self.throttle_position.clamp(0.0, 1.0);
        let requested_air = if overrun_fuel_cut {
            0.0
        } else {
            self.throttle_position.max(idle_throttle).clamp(0.0, 1.0)
        };
        let speed_vacuum_factor = (rpm / self.config.idle_rpm).clamp(0.0, 1.0);
        let manifold_target = self.config.ambient_pressure_kpa
            - (self.config.ambient_pressure_kpa - self.config.idle_manifold_pressure_kpa)
                * (1.0 - requested_air.powf(0.65))
                * speed_vacuum_factor;
        let manifold_response =
            1.0 - (-dt_seconds / self.config.manifold_fill_seconds.max(f64::EPSILON)).exp();
        self.manifold_pressure_kpa +=
            (manifold_target - self.manifold_pressure_kpa) * manifold_response;
        self.manifold_pressure_kpa = self.manifold_pressure_kpa.clamp(
            self.config.idle_manifold_pressure_kpa,
            self.config.ambient_pressure_kpa,
        );
        let manifold_air_fraction = ((self.manifold_pressure_kpa
            - self.config.idle_manifold_pressure_kpa)
            / (self.config.ambient_pressure_kpa - self.config.idle_manifold_pressure_kpa))
            .clamp(0.0, 1.0);
        let effective_throttle = manifold_air_fraction.max(idle_throttle).clamp(0.0, 1.0);
        (effective_throttle, manifold_air_fraction)
    }

    /// Returns the power contribution of the firing events in one 720-degree cycle.
    ///
    /// A four-stroke cylinder produces useful pressure for roughly one 180-degree
    /// power stroke after ignition. The pulse is normalised to a mean of one over
    /// a full 720-degree cycle, so `max_torque_nm` remains calibrated as a mean
    /// torque rather than being reduced by the duty cycle of the power stroke.
    fn four_stroke_power_factor(&self, cycle_angle_rad: f64) -> f64 {
        let firing_count = usize::from(self.config.cylinders).min(8);
        let mut firing_offsets = [0.0; 8];
        if self.config.layout == "parallel_twin_270" && self.config.cylinders == 2 {
            firing_offsets[0] = 0.0;
            firing_offsets[1] = TAU * 0.75;
        } else {
            let cylinder_count = u32::try_from(firing_count.max(1)).unwrap_or(1);
            for (index, offset) in firing_offsets.iter_mut().enumerate().take(firing_count) {
                let index = u32::try_from(index).unwrap_or(0);
                *offset = FOUR_STROKE_CYCLE_RADIANS * f64::from(index) / f64::from(cylinder_count);
            }
        }
        let mut pulse = 0.0;
        for offset in firing_offsets.into_iter().take(firing_count) {
            let phase = (cycle_angle_rad - offset).rem_euclid(FOUR_STROKE_CYCLE_RADIANS);
            if phase < POWER_STROKE_RADIANS {
                pulse += (phase / POWER_STROKE_RADIANS * std::f64::consts::PI)
                    .sin()
                    .max(0.0);
            }
        }
        let cylinder_count = u32::try_from(firing_count.max(1)).unwrap_or(1);
        // Each half-sine power event spans a quarter of a 720-degree cycle, so
        // its analytical mean is 1 / (2π). Summing all cylinders gives n / (2π).
        let mean_pulse_sum = f64::from(cylinder_count) / TAU;
        let normalised_pulse = pulse / mean_pulse_sum;
        0.72 + 0.28 * normalised_pulse
    }

    fn torque_curve(&self, rpm: f64) -> f64 {
        let distance = (rpm - self.config.peak_torque_rpm) / (self.config.redline_rpm * 0.48);
        (1.0 - 0.48 * distance * distance).clamp(0.32, 1.0)
    }

    fn friction_torque(&self, rpm: f64) -> f64 {
        let speed_fraction = (rpm / self.config.redline_rpm).clamp(0.0, 1.0);
        let mechanical = self.config.friction_nm_at_idle
            + (self.config.friction_nm_at_redline - self.config.friction_nm_at_idle)
                * speed_fraction;
        if rpm <= 0.0 { 0.0 } else { mechanical }
    }

    fn pumping_torque(&self, rpm: f64, throttle: f64) -> f64 {
        if rpm <= 0.0 {
            return 0.0;
        }
        let speed_fraction = (rpm / self.config.redline_rpm).clamp(0.0, 1.0);
        self.config.effective_max_pumping_brake_nm()
            * (1.0 - throttle).powi(2)
            * speed_fraction.powf(1.15)
    }

    fn combustion_stability_rpm(&self) -> f64 {
        (self.config.idle_rpm * 0.45).max(COMBUSTION_CUTOFF_RPM)
    }

    fn resolve_stall(
        &mut self,
        dt_seconds: f64,
        mut next_rpm: f64,
        combustion_torque: f64,
        starter_torque: f64,
        engine_braking_torque: f64,
        clutch_torque: f64,
    ) -> f64 {
        // Clutch release can load the crank faster than the idle controller and
        // four-stroke combustion can recover it. Sustained sub-idle operation
        // under a positive clutch load extinguishes combustion. The exposure
        // time avoids turning a short torque-pulse dip into an artificial stall.
        let clutch_load_torque = clutch_torque.max(0.0);
        let resisting_torque = engine_braking_torque + clutch_load_torque;
        let combustion_support_torque = combustion_torque + starter_torque;
        let low_speed_load = self.inputs.gear > 0
            && self.inputs.clutch_engagement >= 0.20
            && clutch_load_torque > 0.0
            && resisting_torque > combustion_support_torque;
        let stability_rpm = self.combustion_stability_rpm();
        if !self.inputs.starter && low_speed_load && next_rpm < stability_rpm {
            self.stall_exposure_seconds += dt_seconds;
        } else if next_rpm >= stability_rpm || !low_speed_load {
            self.stall_exposure_seconds = 0.0;
        }

        let combustion_failed = !self.inputs.starter
            && low_speed_load
            && (next_rpm < COMBUSTION_CUTOFF_RPM
                || self.stall_exposure_seconds >= STALL_EXPOSURE_SECONDS);
        if combustion_failed {
            next_rpm = 0.0;
            self.stall_exposure_seconds = 0.0;
            self.stalled = true;
        } else if self.inputs.ignition
            && next_rpm >= COMBUSTION_FULL_RPM
            && (self.inputs.starter || self.stalled)
        {
            // Cranking or a sufficiently fast bump-start restores stable firing.
            self.stalled = false;
            self.stall_exposure_seconds = 0.0;
        }
        next_rpm
    }

    fn clutch_torque(&self, crank_angular_speed: f64) -> f64 {
        let Some(ratio) = self.config.gearbox.overall_ratio(self.inputs.gear) else {
            return 0.0;
        };
        if self.inputs.clutch_engagement <= 0.0 {
            return 0.0;
        }
        let slip = crank_angular_speed - self.wheel_angular_velocity_rad_s * ratio;
        let capacity = self.config.gearbox.clutch_capacity_nm * self.inputs.clutch_engagement;
        (slip * self.config.gearbox.clutch_stiffness_nm_per_rad_s).clamp(-capacity, capacity)
    }

    fn drivetrain_wheel_torque(&self, clutch_torque: f64) -> f64 {
        self.config
            .gearbox
            .overall_ratio(self.inputs.gear)
            .map_or(0.0, |ratio| {
                clutch_torque * ratio * self.config.gearbox.transmission_efficiency
            })
    }

    fn aero_drag_torque(&self) -> f64 {
        let speed_fraction = (self.wheel_angular_velocity_rad_s
            * self.config.gearbox.rear_wheel_radius_m
            / (100.0 / 3.6))
            .max(0.0);
        self.config.gearbox.aero_drag_nm_at_100_kph * speed_fraction.powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineInputs, EngineSimulation, FOUR_STROKE_CYCLE_RADIANS};
    use crate::config::EngineConfig;

    fn simulation() -> EngineSimulation {
        EngineSimulation::new(EngineConfig::load_default().expect("valid profile"))
    }

    #[test]
    fn starter_turns_engine() {
        let mut engine = simulation();
        engine.set_inputs(EngineInputs {
            starter: true,
            ..EngineInputs::default()
        });
        for _ in 0..500 {
            engine.step(0.001);
        }
        assert!(engine.state().rpm > 200.0);
    }

    #[test]
    fn inputs_are_clamped() {
        let mut engine = simulation();
        engine.set_inputs(EngineInputs {
            throttle: 2.0,
            ..EngineInputs::default()
        });
        assert!((engine.inputs().throttle - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn four_stroke_power_factor_has_a_unit_cycle_mean() {
        let engine = simulation();
        let samples = 720_u16;
        let mean = (0..samples)
            .map(|index| {
                let angle = FOUR_STROKE_CYCLE_RADIANS * f64::from(index) / f64::from(samples);
                engine.four_stroke_power_factor(angle)
            })
            .sum::<f64>()
            / f64::from(samples);

        assert!((mean - 1.0).abs() < 0.01, "mean factor={mean:.4}");
    }
}
