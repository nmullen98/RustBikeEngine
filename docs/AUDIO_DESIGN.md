# Audio design

## Purpose and boundary

The lab uses procedural sound so the audible result follows the same RPM, load, gear and fuel-cut
state as the simulation. It is intentionally a responsive educational approximation, not a sampled
recording or an acoustic model of a particular exhaust system.

## Acoustic model

For a four-stroke engine, firing frequency is:

```text
firing Hz = (RPM / 60) × cylinders / 2
```

The bundled inline-four therefore fires at 50 Hz at 1,500 rpm and 400 Hz at 12,000 rpm. The
synthesiser makes this firing order and its second/third harmonics the tonal core, so perceived
pitch rises directly with RPM. Exhaust and intake resonators are modulated by firing frequency,
while a weaker crank-order mechanical layer remains audible even without combustion.

At closed throttle, fuel-cut stops combustion noise but not engine noise. Every firing-order
interval re-excites a quieter residual layer representing pumping, exhaust-gas movement and
valvetrain/rotating noise. It has a longer decay than the combustion transient. Rare deterministic
overrun pops remain a small texture rather than replacing the continuous deceleration sound.

The live audio callback uses atomics only. It allocates no memory, takes no locks, and performs no
file, log or network I/O.

## Research basis

- A four-stroke inline-four has four firing events per two crank revolutions, making the firing
  rate the second engine order; engine-order frequency is proportional to RPM. This directly
  motivates the order-based pitch calculation. [Applied Sciences engine-order analysis](https://www.mdpi.com/2076-3417/16/2/616)
- Measured propulsion synthesis treats engine speed and load as time-varying inputs, and combines
  tonal order content with broadband components. This motivates smoothing live RPM/load and keeping
  tonal, resonant and noise layers separate. [Auralization of Accelerating Passenger Cars](https://www.mdpi.com/2076-3417/6/1/5)
- Motorcycle engine-noise work separates combustion, combustion-induced mechanical and mechanical
  sources; combustion is not the whole running-engine sound. This motivates an audible non-combustion
  layer during fuel-cut overrun. [SAE motorcycle combustion-noise study](https://saemobilus.sae.org/papers/19-separation-combustion-noise-using-transient-noise-generation-model-2002-32-1788)

## Tests

`src/audio.rs` tests verify the inline-four firing layout, linear RPM-to-firing-frequency mapping,
and non-zero output during closed-throttle high-RPM overrun. These checks protect the two audible
behaviours most likely to regress: rising pitch and residual deceleration sound.

## Next calibration step

To make the sound specific to a real bike, record legally usable microphone data at known RPM and
load points. Derive order amplitudes and resonant response from that data, record microphone
position and test conditions, then compare generated and recorded spectra. Do not replace this
model with unlicensed online recordings.
