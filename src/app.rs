use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use motorbike_engine_sim::{
    audio::AudioEngine, config::EngineConfig, simulation::EngineSimulation,
};
use std::time::Instant;

const FIXED_STEP_SECONDS: f64 = 0.001;
const MAX_FRAME_SECONDS: f64 = 0.05;

#[allow(clippy::struct_excessive_bools)]
pub struct EngineApp {
    simulation: EngineSimulation,
    audio: Option<AudioEngine>,
    audio_error: Option<String>,
    last_frame: Instant,
    accumulator: f64,
    paused: bool,
    draft_config: EngineConfig,
    config_dirty: bool,
    config_message: Option<(bool, String)>,
    keyboard_clutch_active: bool,
    clutch_before_keyboard: Option<f64>,
}

impl EngineApp {
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        configure_style(&context.egui_ctx);
        let config = EngineConfig::load_default().expect("bundled engine profile must be valid");
        let (audio, audio_error) = match AudioEngine::start(&config) {
            Ok(audio) => (Some(audio), None),
            Err(error) => (None, Some(error)),
        };
        let draft_config = config.clone();
        Self {
            simulation: EngineSimulation::new(config),
            audio,
            audio_error,
            last_frame: Instant::now(),
            accumulator: 0.0,
            paused: false,
            draft_config,
            config_dirty: false,
            config_message: None,
            keyboard_clutch_active: false,
            clutch_before_keyboard: None,
        }
    }

    fn advance_simulation(&mut self) {
        let now = Instant::now();
        let frame_seconds = (now - self.last_frame).as_secs_f64().min(MAX_FRAME_SECONDS);
        self.last_frame = now;
        if self.paused {
            self.accumulator = 0.0;
            return;
        }
        self.accumulator += frame_seconds;
        while self.accumulator >= FIXED_STEP_SECONDS {
            self.simulation.step(FIXED_STEP_SECONDS);
            self.accumulator -= FIXED_STEP_SECONDS;
        }
    }

    fn handle_keyboard(&mut self, context: &egui::Context) {
        let keyboard_is_available =
            !context.egui_wants_keyboard_input() || self.keyboard_clutch_active;
        if !keyboard_is_available {
            return;
        }
        let (space_down, shift_down, shift_up, throttle_up, throttle_down) =
            context.input(|input| {
                (
                    input.key_down(egui::Key::Space),
                    input.key_pressed(egui::Key::ArrowLeft),
                    input.key_pressed(egui::Key::ArrowRight),
                    input.key_pressed(egui::Key::ArrowUp),
                    input.key_pressed(egui::Key::ArrowDown),
                )
            });
        let mut inputs = self.simulation.inputs();
        if throttle_up {
            inputs.throttle = (inputs.throttle + 0.05).min(1.0);
        }
        if throttle_down {
            inputs.throttle = (inputs.throttle - 0.05).max(0.0);
        }
        let shift_ready = space_down || inputs.clutch_engagement <= 0.1;
        if shift_ready && shift_down && inputs.gear > 0 {
            inputs.gear = inputs.gear.saturating_sub(1);
        }
        if shift_ready && shift_up && inputs.gear < self.simulation.config().gearbox.forward_gears()
        {
            inputs.gear = inputs.gear.saturating_add(1);
        }
        if space_down && !self.keyboard_clutch_active {
            self.clutch_before_keyboard = Some(inputs.clutch_engagement);
        }
        if space_down {
            // Space represents pulling the clutch lever: zero torque
            // transfer makes the gear dogs safe to move.
            inputs.clutch_engagement = 0.0;
        } else if self.keyboard_clutch_active {
            inputs.clutch_engagement = self.clutch_before_keyboard.take().unwrap_or(0.0);
        }
        self.keyboard_clutch_active = space_down;
        self.simulation.set_inputs(inputs);
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let mut inputs = self.simulation.inputs();
        ui.heading("Controls");
        ui.add_space(8.0);
        ui.checkbox(&mut inputs.ignition, "Ignition");
        inputs.starter = ui
            .add_sized(
                [ui.available_width(), 38.0],
                egui::Button::new("Hold starter"),
            )
            .is_pointer_button_down_on();

        ui.add_space(14.0);
        labelled_slider(ui, "Throttle", &mut inputs.throttle, 0.0..=1.0, "%", 100.0);
        ui.small("Road load: fixed tarmac model, 110 kg rear-axle load");

        ui.add_space(14.0);
        ui.label(RichText::new("Transmission").strong());
        labelled_slider(
            ui,
            "Clutch engagement",
            &mut inputs.clutch_engagement,
            0.0..=1.0,
            "%",
            100.0,
        );
        let shift_enabled = inputs.clutch_engagement <= 0.1;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    shift_enabled && inputs.gear > 0,
                    egui::Button::new("Gear −"),
                )
                .clicked()
            {
                inputs.gear = inputs.gear.saturating_sub(1);
            }
            ui.label(
                RichText::new(if inputs.gear == 0 {
                    "N".to_owned()
                } else {
                    inputs.gear.to_string()
                })
                .size(22.0)
                .strong(),
            );
            if ui
                .add_enabled(
                    shift_enabled && inputs.gear < self.simulation.config().gearbox.forward_gears(),
                    egui::Button::new("Gear +"),
                )
                .clicked()
            {
                inputs.gear = inputs.gear.saturating_add(1);
            }
        });
        clutch_hint(ui, shift_enabled, inputs.gear);
        ui.small("Keyboard: Up/Down = throttle, Space = pull clutch, Left/Right = shift");
        self.simulation.set_inputs(inputs);

        ui.add_space(18.0);
        self.calibration_controls(ui);

        ui.add_space(18.0);
        ui.separator();
        ui.add_space(10.0);
        ui.label(RichText::new("Simulation").strong());
        ui.checkbox(&mut self.paused, "Pause physics");
        ui.label("Physics: fixed 1 kHz step");
        ui.label(if self.audio.is_some() {
            "Audio: live procedural output"
        } else {
            "Audio: unavailable"
        });
        if let Some(error) = &self.audio_error {
            ui.colored_label(Color32::from_rgb(240, 170, 74), error);
        }
    }

    fn calibration_controls(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.add_space(10.0);
        ui.collapsing("Engine and gearbox setup", |ui| {
            ui.label("Displacement");
            if ui
                .add(
                    egui::DragValue::new(&mut self.draft_config.displacement_cc)
                        .range(49.0..=2500.0)
                        .speed(1.0)
                        .suffix(" cc"),
                )
                .changed()
            {
                self.config_dirty = true;
                self.config_message = None;
            }
            ui.small(format!(
                "Estimated peak torque: {:.1} Nm",
                self.draft_config.effective_max_torque_nm()
            ));

            ui.add_space(10.0);
            ui.label("Gear ratios");
            egui::Grid::new("gear_ratio_editor")
                .num_columns(2)
                .spacing([10.0, 5.0])
                .show(ui, |ui| {
                    for (index, ratio) in
                        self.draft_config.gearbox.gear_ratios.iter_mut().enumerate()
                    {
                        ui.label(format!("Gear {}", index + 1));
                        if ui
                            .add(
                                egui::DragValue::new(ratio)
                                    .range(0.5..=4.0)
                                    .speed(0.01)
                                    .max_decimals(3),
                            )
                            .changed()
                        {
                            self.config_dirty = true;
                            self.config_message = None;
                        }
                        ui.end_row();
                    }
                });
            ui.small("Ratios must decrease from first to sixth.");

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.config_dirty, egui::Button::new("Apply to simulation"))
                    .clicked()
                {
                    match self.simulation.update_config(self.draft_config.clone()) {
                        Ok(()) => {
                            self.config_dirty = false;
                            self.config_message = Some((true, "Changes applied".to_owned()));
                        }
                        Err(error) => {
                            self.config_message = Some((false, error.to_string()));
                        }
                    }
                }
                if ui.button("Reset edits").clicked() {
                    self.draft_config = self.simulation.config().clone();
                    self.config_dirty = false;
                    self.config_message = None;
                }
            });
            if let Some((valid, message)) = &self.config_message {
                ui.colored_label(
                    if *valid {
                        Color32::from_rgb(57, 190, 154)
                    } else {
                        Color32::from_rgb(240, 110, 92)
                    },
                    message,
                );
            }
            ui.small("Applied changes affect this session; the TOML profile remains unchanged.");
        });
    }

    fn dashboard(&self, ui: &mut egui::Ui) {
        let state = self.simulation.state();
        let gearbox = self.simulation.gearbox_state();
        let config = self.simulation.config();
        ui.horizontal(|ui| {
            metric(ui, "ENGINE SPEED", &format!("{:.0} rpm", state.rpm));
            metric(ui, "NET TORQUE", &format!("{:+.1} Nm", state.net_torque_nm));
            metric(
                ui,
                "ENGINE BRAKING",
                &format!("{:.1} Nm", state.engine_braking_torque_nm),
            );
            metric(ui, "STATE", engine_status_label(state));
        });

        ui.add_space(12.0);
        let speed_fraction = visual_f32((state.rpm / config.redline_rpm).clamp(0.0, 1.0));
        ui.add(
            egui::ProgressBar::new(speed_fraction)
                .text(format!("Redline {:.0} rpm", config.redline_rpm))
                .fill(if speed_fraction > 0.92 {
                    Color32::from_rgb(226, 75, 70)
                } else {
                    Color32::from_rgb(57, 190, 154)
                }),
        );
        ui.label(
            RichText::new(format!(
                "FOUR-STROKE CYCLE  •  cylinder 1: {}",
                state.stroke.label()
            ))
            .small()
            .color(Color32::from_rgb(147, 163, 174)),
        );
        ui.label(
            RichText::new(format!(
                "MANIFOLD  •  {:.1} kPa abs  •  throttle plate {:.0}%",
                state.manifold_pressure_kpa,
                state.throttle_position * 100.0
            ))
            .small()
            .color(Color32::from_rgb(147, 163, 174)),
        );
        ui.add_space(14.0);
        let gear_text = if gearbox.selected_gear == 0 {
            "Neutral".to_owned()
        } else {
            format!("Gear {}", gearbox.selected_gear)
        };
        let ratio_text = if gearbox.overall_ratio > 0.0 {
            format!("{:.2}:1", gearbox.overall_ratio)
        } else {
            "—".to_owned()
        };
        ui.horizontal(|ui| {
            metric(ui, "GEAR", &gear_text);
            metric(ui, "OVERALL RATIO", &ratio_text);
            metric(
                ui,
                "WHEEL TORQUE",
                &format!("{:+.0} Nm", gearbox.rear_wheel_torque_nm),
            );
            metric(
                ui,
                "WHEEL SPEED",
                &format!("{:.1} km/h", gearbox.road_speed_kph),
            );
        });
        show_traction_warning(ui, gearbox);
        if gearbox.selected_gear > 0 {
            ui.small(format!(
                "Clutch slip: {:+.0} rpm  •  release the clutch gradually to load the engine",
                gearbox.clutch_slip_rpm
            ));
        }
        ui.add_space(14.0);
        draw_engine(
            ui,
            state.crank_angle_rad,
            state.effective_throttle,
            config.cylinders,
            &config.layout,
        );
        ui.add_space(10.0);
        draw_bike(
            ui,
            gearbox.road_speed_kph,
            gearbox.distance_m,
            state.engine_braking_torque_nm,
            state.effective_throttle,
            gearbox.selected_gear,
        );
        ui.add_space(10.0);
        draw_component_overview(ui, state, gearbox, config);
    }
}

impl eframe::App for EngineApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard(context);
        self.advance_simulation();
        let state = self.simulation.state();
        if let Some(audio) = &self.audio {
            audio.update(
                state,
                self.simulation.gearbox_state(),
                self.simulation.inputs().ignition,
            );
        }
        context.request_repaint_after(std::time::Duration::from_millis(16));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("MOTORBIKE ENGINE LAB");
                ui.separator();
                ui.label(&self.simulation.config().name);
            });
        });
        egui::Panel::left("controls")
            .resizable(false)
            .default_size(260.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui));
            });
        egui::CentralPanel::default().show(ui, |ui| self.dashboard(ui));
    }
}

fn configure_style(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(18, 23, 29);
    visuals.window_fill = Color32::from_rgb(23, 29, 36);
    visuals.selection.bg_fill = Color32::from_rgb(57, 190, 154);
    context.set_visuals(visuals);
}

fn labelled_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
    display_scale: f64,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.monospace(format!("{:.0}{suffix}", *value * display_scale));
        });
    });
    ui.add(egui::Slider::new(value, range).show_value(false));
}

fn clutch_hint(ui: &mut egui::Ui, shift_enabled: bool, gear: u8) {
    if !shift_enabled {
        ui.small("Disengage the clutch below 10% to shift");
    } else if gear > 0 {
        ui.small("Clutch open: engine is isolated from the wheel");
    }
}

fn metric(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.group(|ui| {
        ui.set_min_width(150.0);
        ui.label(RichText::new(label).small().color(Color32::from_gray(150)));
        ui.label(RichText::new(value).size(20.0).strong());
    });
}

fn engine_status_label(state: motorbike_engine_sim::simulation::EngineState) -> &'static str {
    if state.stalled {
        "Stalled — pull clutch, hold Starter"
    } else if state.is_running() {
        "Running"
    } else {
        "Stopped / cranking"
    }
}

fn draw_engine(ui: &mut egui::Ui, crank_angle: f64, throttle: f64, cylinders: u8, layout: &str) {
    let available = ui.available_size();
    let size = Vec2::new(available.x, 330.0);
    let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(10),
        Color32::from_rgb(13, 17, 22),
    );

    let centre_x = rect.center().x;
    let crank_y = rect.bottom() - 88.0;
    let crank_radius = 38.0;
    painter.circle_stroke(
        egui::pos2(centre_x, crank_y),
        crank_radius,
        Stroke::new(4.0, Color32::from_rgb(92, 104, 116)),
    );

    let crank_angle = visual_f32(crank_angle);
    let cylinder_count = usize::from(cylinders).clamp(1, 8);
    let cylinder_count_f32 = f32::from(u16::try_from(cylinder_count).unwrap_or(1));
    let spacing = (available.x.min(620.0) / cylinder_count_f32).max(66.0);
    let first_x = centre_x - spacing * (cylinder_count_f32 - 1.0) * 0.5;
    for index in 0..cylinder_count {
        let index_f32 = f32::from(u16::try_from(index).unwrap_or(0));
        let x_offset = first_x + spacing * index_f32 - centre_x;
        let phase_offset = if layout == "parallel_twin_270" && cylinder_count == 2 {
            if index == 0 {
                0.0
            } else {
                std::f32::consts::TAU * 0.75
            }
        } else {
            std::f32::consts::TAU * index_f32 / cylinder_count_f32
        };
        let x = centre_x + x_offset;
        let angle = crank_angle + phase_offset;
        let crank_pin = egui::pos2(
            centre_x + angle.sin() * crank_radius,
            crank_y + angle.cos() * crank_radius,
        );
        let piston_y = rect.top() + 88.0 + (1.0 - angle.cos()) * 48.0;
        let piston = egui::Rect::from_center_size(egui::pos2(x, piston_y), Vec2::new(78.0, 40.0));
        painter.rect_filled(
            piston,
            egui::CornerRadius::same(4),
            Color32::from_rgb(176, 187, 198),
        );
        painter.line_segment(
            [egui::pos2(x, piston.bottom()), crank_pin],
            Stroke::new(7.0, Color32::from_rgb(120, 132, 144)),
        );
        painter.circle_filled(crank_pin, 8.0, Color32::from_rgb(57, 190, 154));
        painter.text(
            egui::pos2(x, rect.top() + 28.0),
            egui::Align2::CENTER_CENTER,
            format!("CYL {}", index + 1),
            egui::FontId::monospace(14.0),
            Color32::from_gray(185),
        );
    }

    painter.text(
        egui::pos2(rect.left() + 20.0, rect.bottom() - 22.0),
        egui::Align2::LEFT_BOTTOM,
        format!("Effective throttle {:.0}%", throttle * 100.0),
        egui::FontId::monospace(13.0),
        Color32::from_gray(155),
    );
}

fn draw_component_overview(
    ui: &mut egui::Ui,
    state: motorbike_engine_sim::simulation::EngineState,
    gearbox: motorbike_engine_sim::simulation::GearboxState,
    config: &EngineConfig,
) {
    ui.label(RichText::new("Component flow").strong());
    let width = ui.available_width();
    let (response, painter) = ui.allocate_painter(Vec2::new(width, 188.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(10),
        Color32::from_rgb(13, 17, 22),
    );

    let labels = [
        (
            "AIRBOX / THROTTLE",
            format!("{:.0}%", state.throttle_position * 100.0),
        ),
        ("INLINE-4", format!("{:+.1} Nm", state.combustion_torque_nm)),
        ("CLUTCH", format!("{:+.0} rpm", gearbox.clutch_slip_rpm)),
        (
            "GEARBOX",
            if gearbox.selected_gear == 0 {
                "NEUTRAL".to_owned()
            } else {
                format!("G{}  {:.2}:1", gearbox.selected_gear, gearbox.overall_ratio)
            },
        ),
        (
            "CHAIN / FINAL",
            if gearbox.traction_limited {
                format!(
                    "{:+.0} / {:+.0} Nm",
                    gearbox.rear_wheel_torque_nm, gearbox.requested_rear_wheel_torque_nm
                )
            } else {
                format!("{:+.0} Nm", gearbox.rear_wheel_torque_nm)
            },
        ),
        ("REAR TYRE", format!("{:.1} km/h", gearbox.road_speed_kph)),
        (
            "TARMAC",
            format!("{:.1} Nm", config.gearbox.static_tarmac_load_torque_nm()),
        ),
    ];
    let gap = 8.0;
    let node_width = ((rect.width() - gap * 6.0 - 20.0) / 7.0).max(68.0);
    let node_height = 62.0;
    let top = rect.top() + 25.0;
    for (index, (label, value)) in labels.iter().enumerate() {
        let index_f32 = f32::from(u16::try_from(index).unwrap_or(0));
        let left = rect.left() + 10.0 + index_f32 * (node_width + gap);
        let node =
            egui::Rect::from_min_size(egui::pos2(left, top), Vec2::new(node_width, node_height));
        let accent = if index == 1 && !state.is_running() {
            Color32::from_rgb(226, 75, 70)
        } else if (index == 2 && gearbox.clutch_slip_rpm.abs() > 500.0)
            || (index == 4 && gearbox.traction_limited)
        {
            Color32::from_rgb(240, 170, 74)
        } else {
            Color32::from_rgb(57, 190, 154)
        };
        component_node(&painter, node, label, value, accent);
        if index + 1 < labels.len() {
            let next_left = left + node_width + gap;
            draw_arrow(
                &painter,
                egui::pos2(left + node_width + 1.0, node.center().y),
                egui::pos2(next_left - 2.0, node.center().y),
            );
        }
    }

    let exhaust = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 10.0 + (node_width + gap), top + 82.0),
        Vec2::new(node_width, 42.0),
    );
    component_node(
        &painter,
        exhaust,
        "EXHAUST",
        &format!(
            "{:.0} / {:.0} Hz",
            config.exhaust_primary_hz, config.exhaust_secondary_hz
        ),
        Color32::from_rgb(224, 92, 64),
    );
    draw_arrow(
        &painter,
        egui::pos2(
            rect.left() + 10.0 + (node_width + gap) + node_width * 0.5,
            top + node_height,
        ),
        exhaust.center_top(),
    );
}

fn show_traction_warning(
    ui: &mut egui::Ui,
    gearbox: motorbike_engine_sim::simulation::GearboxState,
) {
    if gearbox.traction_limited {
        ui.colored_label(
            Color32::from_rgb(240, 170, 74),
            format!(
                "Traction limit: requested {:+.0} Nm, applied {:+.0} Nm",
                gearbox.requested_rear_wheel_torque_nm, gearbox.rear_wheel_torque_nm
            ),
        );
    }
}

fn component_node(
    painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    value: &str,
    accent: Color32,
) {
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(6),
        Color32::from_rgb(24, 31, 39),
    );
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(6),
        Stroke::new(1.5, accent),
        egui::StrokeKind::Inside,
    );
    painter.text(
        egui::pos2(rect.center().x, rect.top() + 18.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::monospace(10.0),
        Color32::from_gray(175),
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 18.0),
        egui::Align2::CENTER_CENTER,
        value,
        egui::FontId::monospace(12.0),
        Color32::from_gray(225),
    );
}

fn draw_arrow(painter: &egui::Painter, start: egui::Pos2, end: egui::Pos2) {
    let colour = Color32::from_rgb(92, 104, 116);
    painter.line_segment([start, end], Stroke::new(1.5, colour));
    painter.line_segment(
        [end, egui::pos2(end.x - 5.0, end.y - 4.0)],
        Stroke::new(1.5, colour),
    );
    painter.line_segment(
        [end, egui::pos2(end.x - 5.0, end.y + 4.0)],
        Stroke::new(1.5, colour),
    );
}

#[derive(Clone, Copy)]
struct BikeVisualState {
    rear: egui::Pos2,
    front: egui::Pos2,
    bike_x: f32,
    road_y: f32,
    suspension_bob: f32,
    wheel_rotation: f32,
    engine_braking_nm: f64,
    throttle: f64,
    road_speed_kph: f64,
}

fn draw_bike(
    ui: &mut egui::Ui,
    road_speed_kph: f64,
    distance_m: f64,
    engine_braking_nm: f64,
    throttle: f64,
    gear: u8,
) {
    let width = ui.available_width();
    let (response, painter) = ui.allocate_painter(Vec2::new(width, 190.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(10),
        Color32::from_rgb(11, 15, 19),
    );
    let road_y = rect.bottom() - 47.0;
    draw_ground(&painter, rect, road_y, distance_m);

    // The bike is deliberately fixed at the centre; only the world moves.
    let bike_x = rect.center().x;
    let motion = visual_f32((distance_m * 0.75).sin());
    let suspension_bob = if road_speed_kph > 0.2 {
        motion * (1.2 + visual_f32((engine_braking_nm / 40.0).clamp(0.0, 1.0)) * 1.8)
    } else {
        0.0
    };
    let wheel_radius = 23.0;
    let rear = egui::pos2(bike_x - 44.0, road_y - wheel_radius - suspension_bob);
    let front = egui::pos2(bike_x + 44.0, road_y - wheel_radius - suspension_bob);
    let wheel_rotation = visual_f32(distance_m / 0.315);

    painter.line_segment(
        [
            egui::pos2(rear.x - 30.0, road_y - 1.0),
            egui::pos2(front.x + 30.0, road_y - 1.0),
        ],
        Stroke::new(8.0, Color32::from_rgba_unmultiplied(0, 0, 0, 85)),
    );
    draw_wheel(&painter, rear, wheel_radius, wheel_rotation);
    draw_wheel(&painter, front, wheel_radius, wheel_rotation);

    draw_bike_mechanics(
        &painter,
        BikeVisualState {
            rear,
            front,
            bike_x,
            road_y,
            suspension_bob,
            wheel_rotation,
            engine_braking_nm,
            throttle,
            road_speed_kph,
        },
    );

    painter.text(
        egui::pos2(rect.left() + 16.0, rect.top() + 18.0),
        egui::Align2::LEFT_TOP,
        if road_speed_kph > 0.1 {
            format!("LIVE BIKE  •  {road_speed_kph:.1} km/h  •  G{gear}")
        } else {
            format!("LIVE BIKE  •  STATIONARY  •  G{gear}")
        },
        egui::FontId::monospace(12.0),
        Color32::from_gray(155),
    );
}

fn draw_bike_mechanics(painter: &egui::Painter, visual: BikeVisualState) {
    let BikeVisualState {
        rear,
        front,
        bike_x,
        road_y,
        suspension_bob,
        wheel_rotation,
        engine_braking_nm,
        throttle,
        road_speed_kph,
    } = visual;
    let frame_colour = Color32::from_rgb(224, 92, 64);
    let engine = egui::Rect::from_center_size(
        egui::pos2(bike_x - 1.0, road_y - 43.0 - suspension_bob),
        Vec2::new(48.0, 31.0),
    );
    painter.rect_filled(
        engine,
        egui::CornerRadius::same(5),
        Color32::from_rgb(57, 190, 154),
    );
    painter.line_segment(
        [
            egui::pos2(rear.x, rear.y - 3.0),
            egui::pos2(bike_x - 9.0, road_y - 78.0 - suspension_bob),
        ],
        Stroke::new(6.0, frame_colour),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x - 9.0, road_y - 78.0 - suspension_bob),
            egui::pos2(front.x, front.y - 4.0),
        ],
        Stroke::new(6.0, frame_colour),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x - 9.0, road_y - 78.0 - suspension_bob),
            egui::pos2(bike_x + 23.0, road_y - 42.0 - suspension_bob),
        ],
        Stroke::new(5.0, frame_colour),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x + 23.0, road_y - 42.0 - suspension_bob),
            egui::pos2(front.x, front.y - 4.0),
        ],
        Stroke::new(4.0, Color32::from_rgb(172, 185, 194)),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x + 27.0, road_y - 69.0 - suspension_bob),
            egui::pos2(bike_x + 49.0, road_y - 83.0 - suspension_bob),
        ],
        Stroke::new(4.0, Color32::from_rgb(172, 185, 194)),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x + 39.0, road_y - 84.0 - suspension_bob),
            egui::pos2(bike_x + 61.0, road_y - 84.0 - suspension_bob),
        ],
        Stroke::new(3.0, Color32::from_rgb(172, 185, 194)),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x - 29.0, road_y - 55.0 - suspension_bob),
            egui::pos2(bike_x + 23.0, road_y - 55.0 - suspension_bob),
        ],
        Stroke::new(8.0, Color32::from_rgb(45, 53, 62)),
    );
    draw_chain(
        painter,
        rear,
        egui::pos2(bike_x - 16.0, road_y - 43.0 - suspension_bob),
        wheel_rotation,
    );
    draw_rider(painter, bike_x, road_y - suspension_bob);
    draw_lights(painter, bike_x, road_y, front, engine_braking_nm, throttle);
    draw_exhaust_plume(
        painter,
        bike_x - 37.0,
        road_y - 58.0 - suspension_bob,
        throttle,
        road_speed_kph,
    );
}

fn draw_ground(painter: &egui::Painter, rect: egui::Rect, road_y: f32, distance_m: f64) {
    painter.line_segment(
        [
            egui::pos2(rect.left(), road_y),
            egui::pos2(rect.right(), road_y),
        ],
        Stroke::new(2.0, Color32::from_rgb(70, 82, 92)),
    );
    let scroll = visual_f32((distance_m * 7.0).rem_euclid(168.0));
    let mut x = rect.left() - scroll;
    while x < rect.right() {
        painter.line_segment(
            [
                egui::pos2(x, road_y + 17.0),
                egui::pos2(x + 42.0, road_y + 17.0),
            ],
            Stroke::new(2.0, Color32::from_rgb(105, 116, 124)),
        );
        x += 84.0;
    }
    let texture_scroll = visual_f32((distance_m * 3.0).rem_euclid(48.0));
    let mut texture_x = rect.left() - texture_scroll;
    while texture_x < rect.right() {
        painter.line_segment(
            [
                egui::pos2(texture_x, road_y + 30.0),
                egui::pos2(texture_x + 10.0, road_y + 30.0),
            ],
            Stroke::new(1.0, Color32::from_rgb(48, 58, 67)),
        );
        texture_x += 48.0;
    }
}

fn draw_wheel(painter: &egui::Painter, centre: egui::Pos2, radius: f32, rotation: f32) {
    painter.circle_filled(centre, radius, Color32::from_rgb(20, 24, 29));
    painter.circle_stroke(
        centre,
        radius,
        Stroke::new(3.0, Color32::from_rgb(172, 185, 194)),
    );
    painter.circle_stroke(
        centre,
        radius * 0.34,
        Stroke::new(2.0, Color32::from_rgb(105, 116, 124)),
    );
    for spoke in 0_u16..6 {
        let angle = rotation + f32::from(spoke) * std::f32::consts::TAU / 6.0;
        painter.line_segment(
            [
                centre,
                egui::pos2(
                    centre.x + angle.cos() * radius * 0.82,
                    centre.y + angle.sin() * radius * 0.82,
                ),
            ],
            Stroke::new(1.0, Color32::from_rgb(92, 104, 116)),
        );
    }
}

fn draw_chain(
    painter: &egui::Painter,
    rear: egui::Pos2,
    engine_sprocket: egui::Pos2,
    rotation: f32,
) {
    painter.line_segment(
        [
            egui::pos2(rear.x, rear.y),
            egui::pos2(engine_sprocket.x, engine_sprocket.y),
        ],
        Stroke::new(2.0, Color32::from_rgb(190, 197, 203)),
    );
    for index in 0_u16..8 {
        let fraction = (f32::from(index) + rotation.rem_euclid(8.0) / 8.0) / 8.0;
        let fraction = fraction.rem_euclid(1.0);
        let point = rear + (engine_sprocket - rear) * fraction;
        painter.circle_filled(point, 1.6, Color32::from_rgb(224, 92, 64));
    }
}

fn draw_rider(painter: &egui::Painter, bike_x: f32, road_y: f32) {
    painter.circle_filled(
        egui::pos2(bike_x + 5.0, road_y - 112.0),
        10.0,
        Color32::from_rgb(49, 58, 68),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x + 5.0, road_y - 101.0),
            egui::pos2(bike_x + 15.0, road_y - 71.0),
        ],
        Stroke::new(7.0, Color32::from_rgb(49, 58, 68)),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x + 15.0, road_y - 72.0),
            egui::pos2(bike_x - 4.0, road_y - 48.0),
        ],
        Stroke::new(5.0, Color32::from_rgb(49, 58, 68)),
    );
    painter.line_segment(
        [
            egui::pos2(bike_x + 15.0, road_y - 72.0),
            egui::pos2(bike_x + 39.0, road_y - 84.0),
        ],
        Stroke::new(5.0, Color32::from_rgb(49, 58, 68)),
    );
}

fn draw_lights(
    painter: &egui::Painter,
    bike_x: f32,
    road_y: f32,
    front: egui::Pos2,
    engine_braking_nm: f64,
    throttle: f64,
) {
    let brake_on = engine_braking_nm > 9.0 && throttle < 0.08;
    painter.circle_filled(
        egui::pos2(bike_x - 48.0, road_y - 62.0),
        4.0,
        if brake_on {
            Color32::from_rgb(242, 63, 54)
        } else {
            Color32::from_rgb(105, 36, 36)
        },
    );
    painter.circle_filled(front, 5.0, Color32::from_rgb(247, 218, 128));
    painter.line_segment(
        [front, egui::pos2(front.x + 34.0, front.y - 7.0)],
        Stroke::new(2.0, Color32::from_rgba_unmultiplied(247, 218, 128, 90)),
    );
}

fn draw_exhaust_plume(painter: &egui::Painter, x: f32, y: f32, throttle: f64, road_speed_kph: f64) {
    let strength =
        visual_f32((0.25 + throttle * 0.75) * (1.0 - road_speed_kph / 220.0).clamp(0.25, 1.0));
    for index in 0_u16..4 {
        let index_f32 = f32::from(index);
        painter.circle_filled(
            egui::pos2(x - index_f32 * 10.0, y - index_f32 * 3.0),
            4.0 + index_f32 * 1.8,
            Color32::from_rgba_unmultiplied(150, 160, 170, visual_alpha(45.0 * strength)),
        );
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn visual_alpha(value: f32) -> u8 {
    value.clamp(0.0, 255.0) as u8
}

#[allow(clippy::cast_possible_truncation)]
fn visual_f32(value: f64) -> f32 {
    value as f32
}
