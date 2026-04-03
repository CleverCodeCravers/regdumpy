# regdumpy

Windows-only Rust CLI tool that dumps the live Windows registry to JSONL format.

## Build & Test

```bash
cargo build --release    # Release binary
cargo test               # Run all tests
cargo clippy             # Lint
cargo fmt --check        # Check formatting
```

## Architecture

- `src/main.rs` — CLI entry point (clap argument parsing, elevation check)
- `src/lib.rs` — Module root
- `src/dumper.rs` — Core logic: recursive registry walk, value serialization to JSONL

## Conventions

- Platform: Windows only (uses `winreg` crate)
- Output format: JSON Lines (one registry value per line)
- Error handling: `anyhow::Result`, invalid registry data is marked in output rather than causing crashes
- Requirements are tracked in `docs/requirements/` and `Anforderungen/`
