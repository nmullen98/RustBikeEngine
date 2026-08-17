# Contributing

## Development setup

Install the Rust version selected by `rust-toolchain.toml`, then run:

```sh
cargo run
cargo fmt --check
cargo quality
cargo verify
```

## Change rules

- Keep calculations in SI units and document every new calibration input with its unit.
- Validate every user-editable numeric value before it reaches the solver.
- Keep the audio callback allocation-free, lock-free, and free of file/network I/O.
- Add or update a regression test for every changed physical behaviour.
- Do not add sliders or claim accuracy without source data, calibration conditions and an error metric.
- Do not commit generated binaries, recordings, logs, local environment files, credentials or private datasets.

## Pull requests

Describe the physical assumption, the observable behaviour being changed, the test coverage, and
the calibration source or reasoned approximation. Keep unrelated formatting and refactors separate.

## Commit hygiene

Before committing, run the quality commands above and inspect staged paths with `git diff --cached`.
Use `git grep -n -i` to verify no credential, token, password or private-key material is staged.
