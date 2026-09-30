## Prerequisites

- **Rust** — edition 2024, so a toolchain **≥ 1.85** (CI builds on `beta`).
- **Docker** — for PostgreSQL in dev and for the full prod stack.
- **make** — optional but recommended. On Windows, run it from the dev
  container, WSL, or Git Bash (the recipes are POSIX sh).

## Build

From the repository root, the [`Makefile`](./Makefile) wraps the workspace-wide
cargo flows across all three crates:

```bash
make deps     # fetch every dependency
make build    # build all crates (all targets + features)
make lint     # clippy across the workspace, warnings denied
make test     # workspace tests (needs a reachable DATABASE_URL)
make ci       # fmt-check + lint + build + test, as CI runs them
make help     # list every target
```

Or drive cargo directly: `cargo build --workspace`.
