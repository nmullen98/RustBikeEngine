//! Validated, data-only engine and gearbox calibration profiles.
//!
//! Profiles are deserialised from TOML, then bounded before entering the
//! simulation. No profile value is executed as code.

use serde::Deserialize;
use std::fmt;

/// Editable motorcycle transmission parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct GearboxConfig {
    /// Crankshaft-to-gearbox-input reduction ratio.
    pub primary_reduction: f64,
    /// Forward gear ratios, ordered from first through top gear.
    pub gear_ratios: Vec<f64>,
    /// Front-sprocket-to-rear-sprocket reduction ratio.
    pub final_drive_ratio: f64,
    /// Fraction of drivetrain torque delivered after gearbox and chain losses.
    pub transmission_efficiency: f64,
    /// Loaded rear-tyre rolling radius in metres.
    pub rear_wheel_radius_m: f64,
    /// Static normal load carried by the driven rear tyre.
    pub rear_axle_load_kg: f64,
    /// Dimensionless tyre/tarmac rolling-resistance coefficient.
    pub tyre_rolling_resistance_coefficient: f64,
    /// Dimensionless peak longitudinal friction coefficient for the rear tyre.
    pub tyre_peak_friction_coefficient: f64,
    /// Total motorcycle mass reflected into longitudinal wheel inertia, in kg.
    pub vehicle_mass_kg: f64,
    /// Physical rear-wheel rotational inertia, in kg·m².
    pub wheel_inertia_kg_m2: f64,
    /// Aerodynamic resistance torque at 100 km/h, in N·m.
    pub aero_drag_nm_at_100_kph: f64,
    /// Maximum clutch torque at full lever release, in N·m.
    pub clutch_capacity_nm: f64,
    /// Clutch torque per crank/wheel slip speed, in N·m per rad/s.
    pub clutch_stiffness_nm_per_rad_s: f64,
}

/// Parameters that describe one engine build.
#[derive(Debug, Clone, Deserialize)]
pub struct EngineConfig {
    /// Human-readable profile name shown by the UI.
    pub name: String,
    /// Firing-layout identifier used by the simulation and audio model.
    pub layout: String,
    /// Number of cylinders represented by the profile.
    pub cylinders: u8,
    /// Number of strokes in the engine cycle; the current solver accepts four.
    pub cycle_strokes: u8,
    /// Total swept displacement, in cubic centimetres.
    pub displacement_cc: f64,
    /// Calibration displacement for torque, pumping loss, and rotating inertia values.
    pub reference_displacement_cc: f64,
    /// Cylinder bore, in millimetres.
    pub bore_mm: f64,
    /// Crank stroke, in millimetres.
    pub stroke_mm: f64,
    /// Geometric compression ratio.
    pub compression_ratio: f64,
    /// Target warm idle speed, in revolutions per minute.
    pub idle_rpm: f64,
    /// Hard engine-speed ceiling, in revolutions per minute.
    pub redline_rpm: f64,
    /// Flywheel inertia, in kg·m².
    pub flywheel_inertia_kg_m2: f64,
    /// Other rotating assembly inertia at the reference displacement, in kg·m².
    pub rotating_inertia_kg_m2: f64,
    /// Calibrated mean peak crank torque, in N·m.
    pub max_torque_nm: f64,
    /// Engine speed at the torque-curve peak, in revolutions per minute.
    pub peak_torque_rpm: f64,
    /// Mechanical friction torque at idle speed, in N·m.
    pub friction_nm_at_idle: f64,
    /// Mechanical friction torque at redline, in N·m.
    pub friction_nm_at_redline: f64,
    /// Maximum closed-throttle pumping loss, in N·m.
    pub max_pumping_brake_nm: f64,
    /// Cranking torque supplied by the starter motor, in N·m.
    pub starter_torque_nm: f64,
    /// Minimum idle-controller throttle demand, from zero to one.
    pub idle_base_throttle: f64,
    /// Proportional idle-controller gain, from zero to one.
    pub idle_control_gain: f64,
    /// Ambient pressure used by the simplified intake manifold model.
    pub ambient_pressure_kpa: f64,
    /// Closed-throttle pressure at a warm idle, before engine-speed correction.
    pub idle_manifold_pressure_kpa: f64,
    /// First-order throttle plate response time.
    pub throttle_response_seconds: f64,
    /// First-order intake manifold filling time.
    pub manifold_fill_seconds: f64,
    /// Primary exhaust resonator frequency used by procedural audio, in Hz.
    pub exhaust_primary_hz: f64,
    /// Secondary exhaust resonator frequency used by procedural audio, in Hz.
    pub exhaust_secondary_hz: f64,
    /// Intake resonator frequency used by procedural audio, in Hz.
    pub intake_resonance_hz: f64,
    /// Transmission and tyre calibration paired with this engine profile.
    pub gearbox: GearboxConfig,
}

/// Error returned when a profile cannot be parsed or fails physical bounds checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

impl EngineConfig {
    /// Loads the version-controlled starter engine profile.
    ///
    /// # Errors
    ///
    /// Returns an error if the bundled TOML is malformed or contains an unsafe physical range.
    pub fn load_default() -> Result<Self, ConfigError> {
        let config: Self =
            toml::from_str(include_str!("../assets/engines/inline_four_650.toml"))
                .map_err(|error| ConfigError(format!("cannot parse engine profile: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    /// Checks all values before they enter the solver.
    ///
    /// # Errors
    ///
    /// Returns an error naming the first missing, non-finite, or out-of-range value.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.name.trim().is_empty() || self.layout.trim().is_empty() {
            return Err(ConfigError("engine name and layout are required".into()));
        }
        if !(1..=8).contains(&self.cylinders) {
            return Err(ConfigError("cylinders must be between 1 and 8".into()));
        }
        if self.cycle_strokes != 4 {
            return Err(ConfigError(
                "the current solver supports four-stroke engines only".into(),
            ));
        }
        require_range("displacement_cc", self.displacement_cc, 49.0, 2500.0)?;
        require_range(
            "reference_displacement_cc",
            self.reference_displacement_cc,
            49.0,
            2500.0,
        )?;
        require_range("bore_mm", self.bore_mm, 30.0, 130.0)?;
        require_range("stroke_mm", self.stroke_mm, 30.0, 130.0)?;
        require_range("compression_ratio", self.compression_ratio, 6.0, 16.0)?;
        require_range("idle_rpm", self.idle_rpm, 500.0, 3000.0)?;
        require_range(
            "redline_rpm",
            self.redline_rpm,
            self.idle_rpm * 2.0,
            20_000.0,
        )?;
        require_positive("flywheel_inertia_kg_m2", self.flywheel_inertia_kg_m2)?;
        require_positive("rotating_inertia_kg_m2", self.rotating_inertia_kg_m2)?;
        require_positive("max_torque_nm", self.max_torque_nm)?;
        require_range(
            "peak_torque_rpm",
            self.peak_torque_rpm,
            self.idle_rpm,
            self.redline_rpm,
        )?;
        require_positive("friction_nm_at_idle", self.friction_nm_at_idle)?;
        require_positive("friction_nm_at_redline", self.friction_nm_at_redline)?;
        require_positive("max_pumping_brake_nm", self.max_pumping_brake_nm)?;
        require_positive("starter_torque_nm", self.starter_torque_nm)?;
        require_range("idle_base_throttle", self.idle_base_throttle, 0.0, 0.5)?;
        require_range("idle_control_gain", self.idle_control_gain, 0.0, 1.0)?;
        require_range(
            "ambient_pressure_kpa",
            self.ambient_pressure_kpa,
            90.0,
            110.0,
        )?;
        require_range(
            "idle_manifold_pressure_kpa",
            self.idle_manifold_pressure_kpa,
            15.0,
            80.0,
        )?;
        if self.idle_manifold_pressure_kpa >= self.ambient_pressure_kpa {
            return Err(ConfigError(
                "idle manifold pressure must be below ambient pressure".into(),
            ));
        }
        require_range(
            "throttle_response_seconds",
            self.throttle_response_seconds,
            0.005,
            0.5,
        )?;
        require_range(
            "manifold_fill_seconds",
            self.manifold_fill_seconds,
            0.01,
            2.0,
        )?;
        require_positive("exhaust_primary_hz", self.exhaust_primary_hz)?;
        require_positive("exhaust_secondary_hz", self.exhaust_secondary_hz)?;
        require_positive("intake_resonance_hz", self.intake_resonance_hz)?;
        self.gearbox.validate()?;
        Ok(())
    }

    #[must_use]
    /// Returns the crank inertia including displacement-scaled rotating parts.
    pub fn total_inertia_kg_m2(&self) -> f64 {
        self.flywheel_inertia_kg_m2 + self.rotating_inertia_kg_m2 * self.displacement_scale()
    }

    #[must_use]
    /// Returns peak torque scaled from the reference to the active displacement.
    pub fn effective_max_torque_nm(&self) -> f64 {
        self.max_torque_nm * self.displacement_scale()
    }

    #[must_use]
    /// Returns pumping-brake torque scaled from the reference displacement.
    pub fn effective_max_pumping_brake_nm(&self) -> f64 {
        self.max_pumping_brake_nm * self.displacement_scale()
    }

    #[must_use]
    /// Returns active displacement divided by calibration displacement.
    pub fn displacement_scale(&self) -> f64 {
        self.displacement_cc / self.reference_displacement_cc
    }
}

impl GearboxConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        require_range(
            "gearbox.primary_reduction",
            self.primary_reduction,
            1.0,
            4.0,
        )?;
        if !(4..=8).contains(&self.gear_ratios.len()) {
            return Err(ConfigError(
                "gearbox.gear_ratios must contain between 4 and 8 forward gears".into(),
            ));
        }
        for (index, ratio) in self.gear_ratios.iter().copied().enumerate() {
            require_range(&format!("gearbox.gear_ratios[{index}]"), ratio, 0.5, 4.0)?;
        }
        if !self.gear_ratios.windows(2).all(|pair| pair[0] > pair[1]) {
            return Err(ConfigError(
                "gearbox ratios must decrease from first to top gear".into(),
            ));
        }
        require_range(
            "gearbox.final_drive_ratio",
            self.final_drive_ratio,
            1.5,
            6.0,
        )?;
        require_range(
            "gearbox.transmission_efficiency",
            self.transmission_efficiency,
            0.7,
            1.0,
        )?;
        require_range(
            "gearbox.rear_wheel_radius_m",
            self.rear_wheel_radius_m,
            0.2,
            0.5,
        )?;
        require_range(
            "gearbox.rear_axle_load_kg",
            self.rear_axle_load_kg,
            20.0,
            300.0,
        )?;
        require_range(
            "gearbox.tyre_rolling_resistance_coefficient",
            self.tyre_rolling_resistance_coefficient,
            0.005,
            0.04,
        )?;
        require_range(
            "gearbox.tyre_peak_friction_coefficient",
            self.tyre_peak_friction_coefficient,
            0.4,
            1.5,
        )?;
        require_range("gearbox.vehicle_mass_kg", self.vehicle_mass_kg, 80.0, 500.0)?;
        require_range(
            "gearbox.wheel_inertia_kg_m2",
            self.wheel_inertia_kg_m2,
            0.1,
            10.0,
        )?;
        require_range(
            "gearbox.aero_drag_nm_at_100_kph",
            self.aero_drag_nm_at_100_kph,
            0.0,
            200.0,
        )?;
        require_positive("gearbox.clutch_capacity_nm", self.clutch_capacity_nm)?;
        require_positive(
            "gearbox.clutch_stiffness_nm_per_rad_s",
            self.clutch_stiffness_nm_per_rad_s,
        )?;
        Ok(())
    }

    #[must_use]
    /// Returns the number of configured forward gears.
    pub fn forward_gears(&self) -> u8 {
        u8::try_from(self.gear_ratios.len()).unwrap_or(u8::MAX)
    }

    #[must_use]
    /// Returns the primary × selected gear × final-drive reduction, or `None` for neutral.
    pub fn overall_ratio(&self, gear: u8) -> Option<f64> {
        let index = usize::from(gear.checked_sub(1)?);
        self.gear_ratios
            .get(index)
            .map(|ratio| self.primary_reduction * ratio * self.final_drive_ratio)
    }

    #[must_use]
    /// Returns rear-wheel and reflected vehicle inertia at the wheel axis, in kg·m².
    pub fn wheel_inertia_kg_m2(&self) -> f64 {
        self.wheel_inertia_kg_m2 + self.vehicle_mass_kg * self.rear_wheel_radius_m.powi(2)
    }

    #[must_use]
    /// Returns static rolling-resistance torque at the rear wheel, in N·m.
    pub fn static_tarmac_load_torque_nm(&self) -> f64 {
        self.rear_axle_load_kg
            * 9.81
            * self.tyre_rolling_resistance_coefficient
            * self.rear_wheel_radius_m
    }

    /// Maximum longitudinal tyre torque from the configured static rear load.
    #[must_use]
    pub fn max_tyre_torque_nm(&self) -> f64 {
        self.rear_axle_load_kg
            * 9.81
            * self.tyre_peak_friction_coefficient
            * self.rear_wheel_radius_m
    }
}

fn require_positive(name: &str, value: f64) -> Result<(), ConfigError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(ConfigError(format!("{name} must be finite and positive")))
    }
}

fn require_range(name: &str, value: f64, minimum: f64, maximum: f64) -> Result<(), ConfigError> {
    if value.is_finite() && (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(ConfigError(format!(
            "{name} must be between {minimum} and {maximum}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::EngineConfig;

    #[test]
    fn bundled_engine_profile_is_valid() {
        let config = EngineConfig::load_default().expect("bundled profile should be valid");
        assert!((config.redline_rpm - 12_000.0).abs() < f64::EPSILON);
        assert!((config.gearbox.static_tarmac_load_torque_nm() - 5.10).abs() < 0.05);
        assert!((config.gearbox.max_tyre_torque_nm() - 357.0).abs() < 1.0);
    }
}
