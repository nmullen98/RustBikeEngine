use crate::config::{ConfigError, EngineConfig};
use std::f64::consts::TAU;

const RPM_PER_RADIAN_PER_SECOND: f64 = 60.0 / TAU;
const COMBUSTION_CUTOFF_RPM: f64 = 280.0;
const COMBUSTION_FULL_RPM: f64 = 650.0;
const FOUR_STROKE_CYCLE_RADIANS: f64 = TAU * 2.0;
const POWER_STROKE_RADIANS: f64 = TAU * 0.5;

/// The four phases of a cylinder's 720-degree four-stroke cycle.
///
/// The dashboard reports cylinder one's phase. Other cylinders use their own
/// firing offset from the selected firing layout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FourStroke {
    #[default]
    Intake,
    Compression,
    Power,
    Exhaust,
}

impl FourStroke {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Intake => "Intake",
            Self::Compression => "Compression",
            Self::Power => "Power",
            Self::Exhaust => "Exhaust",
        }
    }

    #[must_use]
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
    pub ignition: bool,
    pub starter: bool,
    pub throttle: f64,
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
    pub rpm: f64,
    pub crank_angle_rad: f64,
    /// Absolute phase within the 720-degree four-stroke cycle.
    pub cycle_angle_rad: f64,
    /// Current phase for cylinder one's cycle.
    pub stroke: FourStroke,
    /// Delayed throttle plate position, from zero to one.
    pub throttle_position: f64,
    /// Simplified intake manifold absolute pressure.
    pub manifold_pressure_kpa: f64,
    pub combustion_torque_nm: f64,
    pub friction_torque_nm: f64,
    pub pumping_torque_nm: f64,
    pub engine_braking_torque_nm: f64,
    pub clutch_torque_nm: f64,
    pub net_torque_nm: f64,
    pub effective_throttle: f64,
}

/// Derived gearbox and rear-wheel measurements for the current engine state.
#[derive(Debug, Clone, Copy, Default)]
pub struct GearboxState {
    pub selected_gear: u8,
    pub overall_ratio: f64,
    pub output_rpm: f64,
    pub rear_wheel_torque_nm: f64,
    pub road_speed_kph: f64,
    pub clutch_slip_rpm: f64,
}

impl EngineState {
    #[must_use]
    pub fn is_running(self) -> bool {
        self.rpm >= COMBUSTION_FULL_RPM
    }
}

/// Fixed-step rotational engine model.
pub struct EngineSimulation {
    config: EngineConfig,
    state: EngineState,
    inputs: EngineInputs,
    throttle_position: f64,
    manifold_pressure_kpa: f64,
    wheel_angular_velocity_rad_s: f64,
}

impl EngineSimulation {
    #[must_use]
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
        }
    }

    #[must_use]
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    #[must_use]
    pub fn state(&self) -> EngineState {
        self.state
    }

    #[must_use]
    pub fn inputs(&self) -> EngineInputs {
        self.inputs
    }

    pub fn set_inputs(&mut self, mut inputs: EngineInputs) {
        inputs.throttle = inputs.throttle.clamp(0.0, 1.0);
        inputs.gear = inputs.gear.min(self.config.gearbox.forward_gears());
        inputs.clutch_engagement = inputs.clutch_engagement.clamp(0.0, 1.0);
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
    pub fn gearbox_state(&self) -> GearboxState {
        let wheel_rpm = self.wheel_angular_velocity_rad_s * RPM_PER_RADIAN_PER_SECOND;
        let Some(ratio) = self.config.gearbox.overall_ratio(self.inputs.gear) else {
            return GearboxState {
                road_speed_kph: wheel_rpm * TAU * self.config.gearbox.rear_wheel_radius_m * 60.0
                    / 1000.0,
                ..GearboxState::default()
            };
        };
        let output_rpm = wheel_rpm;
        let wheel_torque =
            self.state.clutch_torque_nm * ratio * self.config.gearbox.transmission_efficiency;
        let road_speed_kph =
            wheel_rpm * TAU * self.config.gearbox.rear_wheel_radius_m * 60.0 / 1000.0;
        let clutch_slip_rpm = self.state.rpm - wheel_rpm * ratio;
        GearboxState {
            selected_gear: self.inputs.gear,
            overall_ratio: ratio,
            output_rpm,
            rear_wheel_torque_nm: wheel_torque,
            road_speed_kph,
            clutch_slip_rpm,
        }
    }

    /// Advances the model. Call with a small fixed `dt_seconds` (1 ms by default).
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

        let combustion_ramp = if self.inputs.ignition && !overrun_fuel_cut {
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
            * cycle_power_factor;
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

        let angular_acceleration = net_torque / self.config.total_inertia_kg_m2();
        let mut angular_speed = crank_angular_speed;
        angular_speed = (angular_speed + angular_acceleration * dt_seconds).max(0.0);
        let mut next_rpm = angular_speed * RPM_PER_RADIAN_PER_SECOND;

        let wheel_drag =
            self.config.gearbox.static_tarmac_load_torque_nm() + self.aero_drag_torque();
        let wheel_angular_acceleration = if self.inputs.gear == 0 {
            -wheel_drag / self.config.gearbox.wheel_inertia_kg_m2()
        } else {
            let ratio = self
                .config
                .gearbox
                .overall_ratio(self.inputs.gear)
                .unwrap_or(0.0);
            (clutch_torque * ratio * self.config.gearbox.transmission_efficiency - wheel_drag)
                / self.config.gearbox.wheel_inertia_kg_m2()
        };
        self.wheel_angular_velocity_rad_s =
            (self.wheel_angular_velocity_rad_s + wheel_angular_acceleration * dt_seconds).max(0.0);

        if next_rpm > self.config.redline_rpm {
            next_rpm = self.config.redline_rpm;
        }
        // Idle control can catch a small dip, but it cannot create enough
        // torque to keep a fully coupled engine turning in a tall gear. Once
        // the crank falls below roughly 35% of idle speed, the combustion
        // cycle loses stability and the engine stalls.
        let fully_coupled = self.inputs.gear > 0 && self.inputs.clutch_engagement >= 0.85;
        if fully_coupled && !self.inputs.starter && next_rpm < self.config.idle_rpm * 0.35 {
            next_rpm = 0.0;
        }
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
        };
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
    /// power stroke after ignition. The small floor keeps the lumped model
    /// controllable between pressure pulses while the phase still drives the
    /// visible torque variation and the clutch/wheel response.
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
        0.52 + 0.48 * (pulse / f64::from(cylinder_count)).min(1.0)
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
    use super::{EngineInputs, EngineSimulation};
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
}
