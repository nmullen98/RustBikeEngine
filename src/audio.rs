//! Allocation-free procedural engine audio for CPAL's real-time callback.

use crate::{
    config::EngineConfig,
    simulation::{EngineState, GearboxState},
};
use cpal::{
    FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    f32::consts::TAU,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

#[derive(Default)]
struct AudioControls {
    rpm_bits: AtomicU32,
    throttle_bits: AtomicU32,
    combustion_bits: AtomicU32,
    braking_bits: AtomicU32,
    output_rpm_bits: AtomicU32,
    ratio_bits: AtomicU32,
    gear_bits: AtomicU32,
    ignition: AtomicBool,
    combusting: AtomicBool,
}

/// Owns the live audio stream. Dropping this stops playback.
pub struct AudioEngine {
    controls: Arc<AudioControls>,
    _stream: Stream,
}

impl AudioEngine {
    /// Starts a procedural stream on the default output device.
    ///
    /// # Errors
    ///
    /// Returns an explanation if no device exists or its format/stream cannot be opened.
    pub fn start(engine: &EngineConfig) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default audio output device is available".to_owned())?;
        let supported = device
            .default_output_config()
            .map_err(|error| format!("cannot read default audio configuration: {error}"))?;
        let sample_format = supported.sample_format();
        let config = supported.config();
        let controls = Arc::new(AudioControls::default());

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, config, &controls, engine),
            SampleFormat::F64 => build_stream::<f64>(&device, config, &controls, engine),
            SampleFormat::I16 => build_stream::<i16>(&device, config, &controls, engine),
            SampleFormat::I32 => build_stream::<i32>(&device, config, &controls, engine),
            SampleFormat::U16 => build_stream::<u16>(&device, config, &controls, engine),
            SampleFormat::U32 => build_stream::<u32>(&device, config, &controls, engine),
            other => Err(format!("unsupported audio sample format: {other}")),
        }?;
        stream
            .play()
            .map_err(|error| format!("cannot start audio stream: {error}"))?;

        Ok(Self {
            controls,
            _stream: stream,
        })
    }

    /// Publishes the latest simulation snapshot to the audio callback without locking.
    ///
    /// This function performs only atomic stores and is safe to call once per GUI frame.
    pub fn update(&self, state: EngineState, gearbox: GearboxState, ignition: bool) {
        self.controls
            .rpm_bits
            .store(audio_f32(state.rpm).to_bits(), Ordering::Relaxed);
        self.controls.throttle_bits.store(
            audio_f32(state.effective_throttle).to_bits(),
            Ordering::Relaxed,
        );
        self.controls.combustion_bits.store(
            audio_f32((state.combustion_torque_nm / 80.0).clamp(0.0, 1.0)).to_bits(),
            Ordering::Relaxed,
        );
        self.controls.braking_bits.store(
            audio_f32((state.engine_braking_torque_nm / 45.0).clamp(0.0, 1.0)).to_bits(),
            Ordering::Relaxed,
        );
        self.controls
            .output_rpm_bits
            .store(audio_f32(gearbox.output_rpm).to_bits(), Ordering::Relaxed);
        self.controls.ratio_bits.store(
            audio_f32(gearbox.overall_ratio).to_bits(),
            Ordering::Relaxed,
        );
        self.controls
            .gear_bits
            .store(u32::from(gearbox.selected_gear), Ordering::Relaxed);
        self.controls.ignition.store(ignition, Ordering::Relaxed);
        self.controls
            .combusting
            .store(state.combustion_torque_nm > 0.1, Ordering::Relaxed);
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    controls: &Arc<AudioControls>,
    engine: &EngineConfig,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut synth = EngineSynth::new(sample_rate_f32(config.sample_rate), engine);
    let controls = Arc::clone(controls);
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                let rpm = f32::from_bits(controls.rpm_bits.load(Ordering::Relaxed));
                let throttle = f32::from_bits(controls.throttle_bits.load(Ordering::Relaxed));
                let combustion = f32::from_bits(controls.combustion_bits.load(Ordering::Relaxed));
                let braking = f32::from_bits(controls.braking_bits.load(Ordering::Relaxed));
                let output_rpm = f32::from_bits(controls.output_rpm_bits.load(Ordering::Relaxed));
                let ratio = f32::from_bits(controls.ratio_bits.load(Ordering::Relaxed));
                let gear = u8::try_from(controls.gear_bits.load(Ordering::Relaxed)).unwrap_or(0);
                let ignition = controls.ignition.load(Ordering::Relaxed);
                let combusting = controls.combusting.load(Ordering::Relaxed);
                synth.render(
                    output,
                    channels,
                    AudioSnapshot {
                        rpm,
                        throttle,
                        combustion,
                        braking,
                        output_rpm,
                        ratio,
                        gear,
                        ignition,
                        combusting,
                    },
                );
            },
            |error| tracing::error!(%error, "audio stream error"),
            None,
        )
        .map_err(|error| format!("cannot create audio stream: {error}"))
}

#[derive(Clone, Copy)]
struct AudioSnapshot {
    rpm: f32,
    throttle: f32,
    combustion: f32,
    braking: f32,
    output_rpm: f32,
    ratio: f32,
    gear: u8,
    ignition: bool,
    combusting: bool,
}

struct EngineSynth {
    sample_rate: f32,
    cylinders: f32,
    primary_hz: f32,
    secondary_hz: f32,
    intake_hz: f32,
    firing_offsets: [f32; 8],
    firing_count: usize,
    cycle_phase: f32,
    primary_phase: f32,
    secondary_phase: f32,
    intake_phase: f32,
    mechanical_phase: f32,
    combustion_envelope: f32,
    exhaust_envelope: f32,
    intake_envelope: f32,
    noise_state: u32,
    event_counter: u32,
    smoothed_rpm: f32,
    smoothed_throttle: f32,
    smoothed_combustion: f32,
    smoothed_braking: f32,
    smoothed_output_rpm: f32,
    smoothed_ratio: f32,
    transmission_phase: f32,
}

impl EngineSynth {
    fn new(sample_rate: f32, engine: &EngineConfig) -> Self {
        let (firing_offsets, firing_count) = firing_pattern(engine);
        Self {
            sample_rate,
            cylinders: f32::from(engine.cylinders),
            primary_hz: audio_f32(engine.exhaust_primary_hz),
            secondary_hz: audio_f32(engine.exhaust_secondary_hz),
            intake_hz: audio_f32(engine.intake_resonance_hz),
            firing_offsets,
            firing_count,
            cycle_phase: 0.0,
            primary_phase: 0.0,
            secondary_phase: 0.0,
            intake_phase: 0.0,
            mechanical_phase: 0.0,
            combustion_envelope: 0.0,
            exhaust_envelope: 0.0,
            intake_envelope: 0.0,
            noise_state: 0x8a5c_39e1,
            event_counter: 0,
            smoothed_rpm: 0.0,
            smoothed_throttle: 0.0,
            smoothed_combustion: 0.0,
            smoothed_braking: 0.0,
            smoothed_output_rpm: 0.0,
            smoothed_ratio: 0.0,
            transmission_phase: 0.0,
        }
    }

    fn render<T>(&mut self, output: &mut [T], channels: usize, snapshot: AudioSnapshot)
    where
        T: Sample + FromSample<f32>,
    {
        let rpm_target = if snapshot.ignition {
            snapshot.rpm
        } else {
            snapshot.rpm.min(500.0)
        };
        let rpm_smoothing = 1.0 - (-1.0 / (self.sample_rate * 0.025)).exp();
        let throttle_smoothing = 1.0 - (-1.0 / (self.sample_rate * 0.012)).exp();
        let load_smoothing = 1.0 - (-1.0 / (self.sample_rate * 0.020)).exp();

        for frame in output.chunks_mut(channels) {
            self.smoothed_rpm += (rpm_target - self.smoothed_rpm) * rpm_smoothing;
            self.smoothed_throttle +=
                (snapshot.throttle - self.smoothed_throttle) * throttle_smoothing;
            self.smoothed_combustion +=
                (snapshot.combustion - self.smoothed_combustion) * load_smoothing;
            self.smoothed_braking += (snapshot.braking - self.smoothed_braking) * load_smoothing;
            self.smoothed_output_rpm +=
                (snapshot.output_rpm - self.smoothed_output_rpm) * load_smoothing;
            self.smoothed_ratio += (snapshot.ratio - self.smoothed_ratio) * load_smoothing;
            let stereo = self.next_sample(snapshot.ignition, snapshot.combusting, snapshot.gear);
            let mut left_channel = true;
            for channel in frame {
                *channel = T::from_sample(if left_channel { stereo[0] } else { stereo[1] });
                left_channel = !left_channel;
            }
        }
    }

    fn next_sample(&mut self, ignition: bool, combusting: bool, gear: u8) -> [f32; 2] {
        let revolutions_per_second = self.smoothed_rpm / 60.0;
        let firing_hz = revolutions_per_second * self.cylinders * 0.5;
        let previous_phase = self.cycle_phase;
        self.cycle_phase += revolutions_per_second / self.sample_rate;
        let wrapped = self.cycle_phase >= 2.0;
        if wrapped {
            self.cycle_phase -= 2.0;
        }
        let firing_event = self.firing_offsets[..self.firing_count]
            .iter()
            .copied()
            .any(|offset| {
                if wrapped {
                    offset > previous_phase || offset <= self.cycle_phase
                } else {
                    offset > previous_phase && offset <= self.cycle_phase
                }
            });
        if firing_event {
            self.event_counter = self.event_counter.wrapping_add(1);
            if ignition && combusting && self.smoothed_rpm > 260.0 {
                let strength = 0.18 + self.smoothed_combustion * 0.82;
                self.combustion_envelope = strength;
                self.exhaust_envelope = (self.exhaust_envelope + strength * 0.75).min(1.5);
                self.intake_envelope =
                    (self.intake_envelope + strength * self.smoothed_throttle * 0.45).min(1.0);
            } else if ignition
                && self.smoothed_braking > 0.35
                && self.event_counter.is_multiple_of(13)
            {
                // Sparse deterministic overrun pops; fuel-cut overrun is otherwise quiet.
                self.combustion_envelope = self.smoothed_braking * 0.10;
                self.exhaust_envelope =
                    (self.exhaust_envelope + self.smoothed_braking * 0.16).min(0.35);
            }
        }

        self.primary_phase = wrap_phase(
            self.primary_phase + TAU * (self.primary_hz + firing_hz * 0.12) / self.sample_rate,
        );
        self.secondary_phase = wrap_phase(
            self.secondary_phase + TAU * (self.secondary_hz + firing_hz * 0.07) / self.sample_rate,
        );
        self.intake_phase = wrap_phase(
            self.intake_phase + TAU * (self.intake_hz + firing_hz * 0.09) / self.sample_rate,
        );
        self.mechanical_phase =
            wrap_phase(self.mechanical_phase + TAU * revolutions_per_second / self.sample_rate);
        let transmission_hz = if gear == 0 {
            0.0
        } else {
            (self.smoothed_output_rpm / 60.0) * (2.0 + f32::from(gear) * 0.7)
                + self.smoothed_ratio * 3.0
        };
        self.transmission_phase =
            wrap_phase(self.transmission_phase + TAU * transmission_hz / self.sample_rate);

        self.combustion_envelope *= (-1.0 / (self.sample_rate * 0.009)).exp();
        self.exhaust_envelope *= (-1.0 / (self.sample_rate * 0.070)).exp();
        self.intake_envelope *= (-1.0 / (self.sample_rate * 0.040)).exp();

        // Xorshift noise supplies the short, broadband combustion transient.
        self.noise_state ^= self.noise_state << 13;
        self.noise_state ^= self.noise_state >> 17;
        self.noise_state ^= self.noise_state << 5;
        let upper_noise = u16::try_from(self.noise_state >> 16).unwrap_or(u16::MAX);
        let noise = (f32::from(upper_noise) / f32::from(u16::MAX)) * 2.0 - 1.0;

        let combustion = noise * self.combustion_envelope * 0.24;
        let exhaust = self.exhaust_envelope
            * (self.primary_phase.sin() * 0.46 + self.secondary_phase.sin() * 0.19);
        let intake = self.intake_envelope * self.intake_phase.sin() * 0.22;
        let speed_level = (self.smoothed_rpm / 10_000.0).clamp(0.0, 1.0);
        let mechanical = (self.mechanical_phase.sin() + (self.mechanical_phase * 2.0).sin() * 0.32)
            * speed_level
            * 0.028;
        let transmission_level = if gear == 0 {
            0.0
        } else {
            (self.smoothed_output_rpm / 1200.0).clamp(0.0, 1.0) * 0.045
        };
        let transmission = self.transmission_phase.sin() * transmission_level;
        let centre = combustion + exhaust + intake + mechanical + transmission;
        let stereo_detail = self.secondary_phase.sin() * self.exhaust_envelope * 0.025;
        [
            soft_clip((centre - stereo_detail) * 0.58),
            soft_clip((centre + stereo_detail) * 0.58),
        ]
    }
}

fn firing_pattern(engine: &EngineConfig) -> ([f32; 8], usize) {
    let mut offsets = [0.0; 8];
    let count = usize::from(engine.cylinders);
    if engine.layout == "parallel_twin_270" && engine.cylinders == 2 {
        offsets[0] = 0.0;
        offsets[1] = 0.75; // 270 degrees, followed by 450 degrees to the next cycle.
    } else {
        let spacing = 2.0 / f32::from(engine.cylinders);
        let mut offset = 0.0;
        for slot in &mut offsets[..count] {
            *slot = offset;
            offset += spacing;
        }
    }
    (offsets, count)
}

fn soft_clip(value: f32) -> f32 {
    value / (1.0 + value.abs())
}

fn wrap_phase(phase: f32) -> f32 {
    if phase >= TAU { phase - TAU } else { phase }
}

/// Audio is deliberately f32; all incoming values have already passed finite bounds validation.
#[allow(clippy::cast_possible_truncation)]
fn audio_f32(value: f64) -> f32 {
    value as f32
}

#[allow(clippy::cast_precision_loss)]
fn sample_rate_f32(value: u32) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::firing_pattern;
    use crate::config::EngineConfig;

    #[test]
    fn inline_four_uses_even_180_degree_firing_pattern() {
        let engine = EngineConfig::load_default().expect("valid engine");
        let (offsets, count) = firing_pattern(&engine);

        assert_eq!(count, 4);
        assert!(offsets[0].abs() < f32::EPSILON);
        assert!((offsets[1] - 0.5).abs() < f32::EPSILON);
        assert!((offsets[2] - 1.0).abs() < f32::EPSILON);
        assert!((offsets[3] - 1.5).abs() < f32::EPSILON);
    }
}
