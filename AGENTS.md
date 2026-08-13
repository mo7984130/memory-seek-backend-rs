# Repository Guidelines

## Project Structure & Module Organization

This Rust 2024 workspace keeps the Axum binary, setup, middleware, configuration, and metrics in `server/`. Business logic is split into `domains/` crates (`auth`, `user`, `photo`, and `backup`). Shared types and utilities live in `types/` and `common/`; reusable integrations are in `libs/`, and development utilities are in `tools/`.

Unit tests are generally colocated with Rust modules. Shared fixtures and k6 load tests live under `tests/`, especially `tests/load/`. ONNX face-recognition assets are stored in `models/` and managed with Git LFS.

## Build, Test, and Development Commands

- `cargo build --features "auth,user,photo"` builds a typical server configuration. Features are opt-in; also available are `metrics`, `face-engine`, and `backup`.
- `cargo run -p server --features "auth,user,photo" -- --config example.config.yaml` runs the API with an explicit configuration file.
- `cargo test --lib` runs the unit-test suite used by CI.
- `cargo fmt --all -- --check` verifies formatting without rewriting files.
- `cargo clippy --all-targets -- -D warnings` treats every Clippy warning as an error.
- `cargo run --release --manifest-path tools/step_boundary_check/Cargo.toml -- --root .` validates pipeline step boundaries.

Copy `example.config.yaml` to an ignored local file and provide the required service settings.

## Coding Style & Naming Conventions

Use standard `rustfmt` output (four-space indentation). Use `snake_case` for modules, functions, and tests; `PascalCase` for types and traits; and `SCREAMING_SNAKE_CASE` for constants. Keep domain behavior in its domain crate and infrastructure wiring in `server/`. CI enforces formatting and Clippy.

## Testing Guidelines

Add focused unit tests beside changed code, with names such as `rejects_expired_token`. Run `cargo test --lib` before opening a PR. Query, cache, or throughput changes should also use the k6 scenarios and Make targets in `tests/load/`; these require Docker and k6. No fixed coverage percentage is enforced, but test new behavior and regressions.

## Commit & Pull Request Guidelines

Use Conventional Commits, matching history: `feat(photo): add face search`, `fix(cache): handle stale entry`, or `refactor(user): extract mapper`. Keep commits focused. PRs target `develop` for CI, explain the change and verification performed, link relevant issues, and call out configuration, schema, or performance impacts. Use `./git-pr.sh <head> <base>` to preview and create a PR after pushing the branch.

## Security & Configuration Tips

Never commit credentials, production configuration, database dumps, or generated test results. Prefer `MEMORY_SEEK_<SECTION>__<KEY>` environment overrides (for example, `MEMORY_SEEK_SERVER__PORT`) and use `RUST_LOG` to adjust logging.
