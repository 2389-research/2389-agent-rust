# Docker Build Notes

## Current Issue: Edition 2024 Dependencies

**Status**: Docker builds blocked by transitive dependencies requiring unstable `edition2024` feature

**Affected Dependencies**:
- `moxcms v0.7.5` (transitive)
- `libxml v0.3.8` (transitive)

**Error**:
```
error: failed to parse manifest at `/usr/local/cargo/registry/.../Cargo.toml`

Caused by:
  feature `edition2024` is required

  The package requires the Cargo feature called `edition2024`, but that feature
  is not stabilized in this version of Cargo (1.83.0).
```

## Temporary Solutions

### Option 1: Build with Nightly Rust

```dockerfile
FROM rust:nightly-slim-bookworm AS builder
ENV CARGO_UNSTABLE_EDITION2024=1
```

### Option 2: Build Locally, Copy Binary

```bash
# Build on host
cargo build --release

# Create minimal runtime image
FROM debian:bookworm-slim
COPY target/release/agent2389 /usr/local/bin/
```

### Option 3: Update Dependencies

Wait for dependencies to drop edition2024 requirement or use stable edition.

## Generic Agent Deployment Pattern

See [GENERIC_AGENT_DEPLOYMENT.md](GENERIC_AGENT_DEPLOYMENT.md) for standardized deployment approach compatible with Fly.io and other platforms.

## Next Steps

1. **Short-term**: Use nightly Rust or local builds
2. **Medium-term**: Wait for edition2024 stabilization (expected Rust 1.86+)
3. **Long-term**: Generic deployment system (see GENERIC_AGENT_DEPLOYMENT.md)
