# Interface wireframes

The editable dashboard wireframe is [`wireframes/dashboard.svg`](wireframes/dashboard.svg).

## Desktop layout

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ MOTORBIKE ENGINE LAB                 650 cc inline four                 │
├─────────────────┬────────────────────────────────────────────────────────┤
│ CONTROLS        │ RPM       NET TORQUE    ENGINE BRAKE    STATE          │
│ [x] Ignition    │ 1,250     +4.2 Nm       6.1 Nm          Running        │
│ [Hold starter]  │ [────────────── redline progress ───────────────]      │
│                 │                                                        │
│ Throttle    18% │        CYL 1   CYL 2   CYL 3   CYL 4                  │
│ ───●─────────── │              [piston]       [piston]                  │
│ Road load fixed │       [piston] [piston] [piston] [piston]             │
│                 │                    ◯─────────                          │
│ Clutch    100%  │ Gear / ratio / wheel torque / road speed              │
│ [Gear −] N [+]  │ Gear / ratio / wheel torque / road speed              │
│ SIMULATION      │                                                        │
│ [ ] Pause       │ Effective throttle 18%                                │
│ Physics: 1 kHz  │                                                        │
│ Audio: live     │                                                        │
└─────────────────┴────────────────────────────────────────────────────────┘
```

The hierarchy prioritises the causal loop: control an input, observe engine speed/torque, and
see the mechanism move. Detailed calibration panels belong in a later inspector, not the primary
driving view.

The drivetrain row exposes the selected gear, total reduction, rear-wheel torque and theoretical
road speed. Shifts require a disengaged clutch so the control teaches the correct causal sequence.
The bike itself stays centred; wheels, chain, suspension, lights, exhaust and tarmac markings move
from the simulated road speed and load state.

The component flow strip below the bike is:

```text
AIRBOX → INLINE-4 → CLUTCH → GEARBOX → CHAIN/FINAL → REAR TYRE → TARMAC
                         ↓
                      EXHAUST
```

Each node shows the live value most useful for diagnosing that component's load or output.
