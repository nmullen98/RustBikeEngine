use serde::Deserialize;
use std::fmt;

/// Editable motorcycle transmission parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct GearboxConfig {
    pub primary_reduction: f64,
    pub gear_ratios: Vec<f64>,
    pub final_drive_ratio: f64,
    pub transmission_efficiency: f64,
    pub rear_wheel_radius_m: f64,
    /// Static normal load carried by the driven rear tyre.
    pub rear_axle_load_kg: f64,
    /// Dimensionless tyre/tarmac rolling-resistance coefficient.
    pub tyre_rolling_resistance_coefficient: f64,
    pub vehicle_mass_kg: f64,
    pub wheel_inertia_kg_m2: f64,
    pub aero_drag_nm_at_100_kph: f64,
    pub clutch_capacity_nm: f64,
    pub clutch_stiffness_nm_per_rad_s: f64,
}

/// Parameters that describe one engine build.
#[derive(Debug, Clone, Deserialize)]
pub struct EngineConfig {
    pub name: String,
    pub layout: String,
    pub cylinders: u8,
    pub cycle_strokes: u8,
    pub displacement_cc: f64,
    /// Calibration displacement for torque, pumping loss, and rotating inertia values.
    pub reference_displacement_cc: f64,
    pub bore_mm: f64,
    pub stroke_mm: f64,
    pub compression_ratio: f64,
    pub idle_rpm: f64,
    pub redline_rpm: f64,
    pub flywheel_inertia_kg_m2: f64,
    pub rotating_inertia_kg_m2: f64,
    pub max_torque_nm: f64,
    pub peak_torque_rpm: f64,
    pub friction_nm_at_idle: f64,
    pub friction_nm_at_redline: f64,
    pub max_pumping_brake_nm: f64,
    pub starter_torque_nm: f64,
    pub idle_base_throttle: f64,
    pub idle_control_gain: f64,
    /// Ambient pressure used by the simplified intake manifold model.
    pub ambient_pressure_kpa: f64,
    /// Closed-throttle pressure at a warm idle, before engine-speed correction.
    pub idle_manifold_pressure_kpa: f64,
    /// First-order throttle plate response time.
    pub throttle_response_seconds: f64,
    /// First-order intake manifold filling time.
    pub manifold_fill_seconds: f64,
    pub exhaust_primary_hz: f64,
    pub exhaust_secondary_hz: f64,
    pub intake_resonance_hz: f64,
    pub gearbox: GearboxConfig,
}

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
    pub fn total_inertia_kg_m2(&self) -> f64 {
        self.flywheel_inertia_kg_m2 + self.rotating_inertia_kg_m2 * self.displacement_scale()
    }

    #[must_use]
    pub fn effective_max_torque_nm(&self) -> f64 {
        self.max_torque_nm * self.displacement_scale()
    }

    #[must_use]
    pub fn effective_max_pumping_brake_nm(&self) -> f64 {
        self.max_pumping_brake_nm * self.displacement_scale()
    }

    #[must_use]
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
    pub fn forward_gears(&self) -> u8 {
        u8::try_from(self.gear_ratios.len()).unwrap_or(u8::MAX)
    }

    #[must_use]
    pub fn overall_ratio(&self, gear: u8) -> Option<f64> {
        let index = usize::from(gear.checked_sub(1)?);
        self.gear_ratios
            .get(index)
            .map(|ratio| self.primary_reduction * ratio * self.final_drive_ratio)
    }

    #[must_use]
    pub fn wheel_inertia_kg_m2(&self) -> f64 {
        self.wheel_inertia_kg_m2 + self.vehicle_mass_kg * self.rear_wheel_radius_m.powi(2)
    }

    #[must_use]
    pub fn static_tarmac_load_torque_nm(&self) -> f64 {
        self.rear_axle_load_kg
            * 9.81
            * self.tyre_rolling_resistance_coefficient
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
    }
}
