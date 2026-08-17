use motorbike_engine_sim::{
    config::EngineConfig,
    simulation::{EngineInputs, EngineSimulation, FourStroke},
};
use std::f64::consts::TAU;

fn run_steps(engine: &mut EngineSimulation, steps: usize) {
    for _ in 0..steps {
        engine.step(0.001);
    }
}

#[test]
fn engine_starts_and_reaches_idle_region() {
    let config = EngineConfig::load_default().expect("valid engine");
    let idle_rpm = config.idle_rpm;
    let mut engine = EngineSimulation::new(config);
    engine.set_inputs(EngineInputs {
        ignition: true,
        starter: true,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 1_200);
    engine.set_inputs(EngineInputs::default());
    run_steps(&mut engine, 4_000);

    assert!(
        engine.state().is_running(),
        "engine settled at {:.1} rpm",
        engine.state().rpm
    );
    assert!((engine.state().rpm - idle_rpm).abs() < idle_rpm * 0.45);
}

#[test]
fn redline_is_a_hard_safety_limit() {
    let config = EngineConfig::load_default().expect("valid engine");
    let redline = config.redline_rpm;
    let mut engine = EngineSimulation::new(config);
    engine.set_inputs(EngineInputs {
        ignition: true,
        starter: true,
        throttle: 1.0,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 20_000);

    assert!(engine.state().rpm <= redline);
}

#[test]
fn lower_gears_multiply_more_torque() {
    let config = EngineConfig::load_default().expect("valid engine");
    let first_ratio = config.gearbox.overall_ratio(1).expect("first gear");
    let top_gear = config.gearbox.forward_gears();
    let top_ratio = config.gearbox.overall_ratio(top_gear).expect("top gear");
    assert!(first_ratio > top_ratio);

    assert!(first_ratio / top_ratio > 2.0);
}

#[test]
fn engaged_gear_couples_engine_rpm_to_wheel_speed() {
    let mut engine = EngineSimulation::new(EngineConfig::load_default().expect("valid engine"));
    engine.set_inputs(EngineInputs {
        ignition: true,
        starter: true,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 1_200);
    engine.set_inputs(EngineInputs {
        gear: 1,
        clutch_engagement: 1.0,
        throttle: 0.45,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 1_000);
    let first_speed = engine.gearbox_state().road_speed_kph;
    let first_rpm = engine.state().rpm;
    assert!(first_speed > 0.5);

    engine.set_inputs(EngineInputs {
        gear: 6,
        clutch_engagement: 0.0,
        throttle: 0.45,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 50);
    engine.set_inputs(EngineInputs {
        gear: 6,
        clutch_engagement: 1.0,
        throttle: 0.45,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 250);
    let top_speed = engine.gearbox_state().road_speed_kph;
    let top_rpm = engine.state().rpm;

    assert!(top_speed > 0.5);
    assert!(top_rpm < first_rpm, "first={first_rpm:.0} top={top_rpm:.0}");
}

#[test]
fn closed_throttle_overrun_cuts_combustion_and_adds_pumping_loss() {
    let config = EngineConfig::load_default().expect("valid engine");
    let mut engine = EngineSimulation::new(config);
    engine.set_inputs(EngineInputs {
        ignition: true,
        starter: true,
        throttle: 1.0,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 3_000);
    assert!(engine.state().rpm > engine.config().idle_rpm * 1.5);

    engine.set_inputs(EngineInputs::default());
    engine.step(0.001);
    let overrun = engine.state();

    assert!(overrun.combustion_torque_nm.abs() < f64::EPSILON);
    assert!(overrun.pumping_torque_nm > 0.0);
    assert!(overrun.engine_braking_torque_nm > overrun.friction_torque_nm);
    assert!(overrun.net_torque_nm < 0.0);
}

#[test]
fn displacement_edit_scales_torque_and_updates_live() {
    let mut config = EngineConfig::load_default().expect("valid engine");
    let original_torque = config.effective_max_torque_nm();
    config.displacement_cc *= 1.25;
    let expected_torque = original_torque * 1.25;

    let mut engine = EngineSimulation::new(EngineConfig::load_default().expect("valid engine"));
    engine.update_config(config).expect("valid live update");

    assert!((engine.config().effective_max_torque_nm() - expected_torque).abs() < f64::EPSILON);
}

#[test]
fn invalid_live_ratio_is_rejected_without_changing_active_config() {
    let config = EngineConfig::load_default().expect("valid engine");
    let original_first = config.gearbox.gear_ratios[0];
    let mut invalid = config.clone();
    invalid.gearbox.gear_ratios[0] = 0.1;
    let mut engine = EngineSimulation::new(config);

    assert!(engine.update_config(invalid).is_err());
    assert!((engine.config().gearbox.gear_ratios[0] - original_first).abs() < f64::EPSILON);
}

#[test]
fn throttle_and_manifold_pressure_have_realistic_lag() {
    let config = EngineConfig::load_default().expect("valid engine");
    let ambient_pressure = config.ambient_pressure_kpa;
    let mut engine = EngineSimulation::new(config);
    engine.set_inputs(EngineInputs {
        ignition: true,
        starter: true,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 2_000);

    engine.set_inputs(EngineInputs {
        ignition: true,
        throttle: 1.0,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 5);
    let transient = engine.state();
    assert!(transient.throttle_position < 0.5);
    assert!(transient.manifold_pressure_kpa < ambient_pressure);

    run_steps(&mut engine, 1_000);
    let settled = engine.state();
    assert!(settled.throttle_position > 0.95);
    assert!(settled.manifold_pressure_kpa > transient.manifold_pressure_kpa);
}

#[test]
fn fully_engaged_top_gear_at_zero_speed_stalls_without_throttle() {
    let mut engine = EngineSimulation::new(EngineConfig::load_default().expect("valid engine"));
    engine.set_inputs(EngineInputs {
        ignition: true,
        starter: true,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 2_000);
    engine.set_inputs(EngineInputs {
        ignition: true,
        gear: 6,
        clutch_engagement: 1.0,
        ..EngineInputs::default()
    });
    run_steps(&mut engine, 2_000);

    assert!(engine.state().rpm < 300.0, "rpm={:.1}", engine.state().rpm);
}

#[test]
fn stationary_clutch_release_reports_gear_load_response() {
    for gear in 1..=6 {
        let mut engine = EngineSimulation::new(EngineConfig::load_default().expect("valid engine"));
        engine.set_inputs(EngineInputs {
            ignition: true,
            starter: true,
            ..EngineInputs::default()
        });
        run_steps(&mut engine, 2_000);
        engine.set_inputs(EngineInputs {
            ignition: true,
            gear,
            clutch_engagement: 1.0,
            ..EngineInputs::default()
        });
        run_steps(&mut engine, 2_000);
        if gear >= 5 {
            assert!(
                engine.state().rpm < 15.0,
                "gear {gear} rpm={:.1}",
                engine.state().rpm
            );
        }
    }
}

#[test]
fn four_stroke_cycle_exposes_four_180_degree_phases() {
    assert_eq!(FourStroke::from_cycle_angle(0.0), FourStroke::Intake);
    assert_eq!(FourStroke::from_cycle_angle(TAU * 0.49), FourStroke::Intake);
    assert_eq!(
        FourStroke::from_cycle_angle(TAU * 0.5),
        FourStroke::Compression
    );
    assert_eq!(FourStroke::from_cycle_angle(TAU), FourStroke::Power);
    assert_eq!(FourStroke::from_cycle_angle(TAU * 1.5), FourStroke::Exhaust);
    assert_eq!(FourStroke::from_cycle_angle(TAU * 2.0), FourStroke::Intake);
}
