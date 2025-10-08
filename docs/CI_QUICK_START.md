# CI/CD Quick Start Guide

## For Developers

### Running Tests Locally (The New Way)

We now use **cargo-nextest** for faster, better test execution:

```bash
# Install nextest (one-time setup)
cargo install cargo-nextest --locked

# Run tests with nextest (2-3x faster!)
cargo nextest run

# Run with all features
cargo nextest run --all-features

# Run specific test
cargo nextest run test_name

# Run with coverage (requires nightly)
rustup toolchain install nightly
cargo +nightly install cargo-llvm-cov --locked
cargo llvm-cov nextest
```

### Using nextest Profiles

```bash
# Fast fail during development
cargo nextest run --profile dev

# CI-like behavior locally
cargo nextest run --profile ci

# Default profile
cargo nextest run
```

### Pre-Commit Checklist

Before pushing your code, run:

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo nextest run --all-features

# Build docs
cargo doc --no-deps --all-features

# Security audit (optional but recommended)
cargo audit
```

Or use a single command:

```bash
# Quick pre-commit check
cargo fmt && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo nextest run --all-features
```

### Understanding CI Job Summaries

After pushing, check the "Summary" tab on your workflow run:

- **Test Results**: Quick pass/fail status
- **Code Coverage**: Coverage percentage and trends
- **Clippy Analysis**: Linting issues found
- **Security Audit**: Vulnerability scan results
- **Compilation Cache**: sccache hit rate (higher = faster builds)

### Troubleshooting CI Failures

#### "Clippy found issues"

Click the clippy job to see details. Fix locally:

```bash
cargo clippy --fix --all-targets --all-features
```

#### "Tests failed"

Check which test failed in the job summary. Run locally:

```bash
cargo nextest run test_name
```

#### "Format check failed"

Fix formatting:

```bash
cargo fmt
```

#### "Security audit found vulnerabilities"

Check the security job for details. Update vulnerable dependencies:

```bash
cargo update
cargo audit
```

### Working with Dependabot PRs

Dependabot will automatically create PRs to update dependencies.

**For patch/minor updates**: Auto-merge if CI passes
**For major updates**: Review changelog and test thoroughly

### Local Development Tools

#### Install recommended tools:

```bash
# Nextest (faster test runner)
cargo install cargo-nextest --locked

# Coverage tool
cargo install cargo-llvm-cov --locked

# Security auditor
cargo install cargo-audit --locked

# Watch for changes (development)
cargo install cargo-watch --locked
```

#### Use cargo-watch for continuous testing:

```bash
# Auto-run tests on file changes
cargo watch -x "nextest run"

# Auto-run clippy on changes
cargo watch -x "clippy --all-targets"

# Combined: fmt + clippy + test
cargo watch -x "fmt" -x "clippy --all-targets" -x "nextest run"
```

## For Maintainers

### Managing the Enhanced Workflow

#### Branch Protection Rules

Set these required checks:
- `CI Success` (this ensures all jobs pass)

Or set individual checks:
- `Test Suite`
- `Code Coverage`
- `Clippy Lints`
- `Rustfmt Check`
- `Security Audit`
- `Documentation`

#### Monitoring CI Performance

Check workflow insights:
- Settings → Actions → Workflow → `Enhanced CI`
- Look for:
  - Average duration trends
  - Success rate
  - Queue times

#### Adjusting sccache Behavior

sccache stats appear in job summaries. If cache hit rate is low (<70%):

1. Check if dependencies changed frequently
2. Verify `SCCACHE_GHA_ENABLED` is set
3. Look for large incremental compilations

#### Enabling Matrix Testing

To test on multiple OS/Rust versions, edit `.github/workflows/ci-enhanced.yml`:

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    rust: [stable, beta]
```

**Note**: This 6x multiplies CI time. Only enable if you need it!

#### Adding New CI Jobs

To add a new job (e.g., benchmarks):

```yaml
benchmarks:
  name: Performance Benchmarks
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo bench
```

Then add to `ci-success` job dependencies:

```yaml
needs: [test, coverage, clippy, format, security, docs, benchmarks]
```

### Debugging CI Issues

#### View sccache logs:

```yaml
- name: Debug sccache
  run: sccache --show-stats
```

#### View nextest configuration:

```yaml
- name: Debug nextest
  run: cargo nextest show-config --profile ci
```

#### Enable verbose logging:

```yaml
env:
  RUST_LOG: debug
  NEXTEST_VERBOSE: true
```

### Cost Optimization

#### Current costs (public repo):
- **$0/month** (unlimited minutes)

#### If migrating to private repo:
- Estimated: ~1,800 min/month
- Free tier: 2,000 min/month
- **Still free!**

#### Tips to reduce CI time:
1. Use concurrency cancel-in-progress ✅ (already enabled)
2. Use sccache ✅ (already enabled)
3. Use nextest ✅ (already enabled)
4. Cache aggressively ✅ (already enabled)
5. Don't run benchmarks on every PR (add manual trigger)

### Release Process (Future)

When we add release automation:

```bash
# Tag a release
git tag v0.2.0
git push origin v0.2.0

# Workflow will:
# 1. Run all CI checks
# 2. Build release artifacts
# 3. Generate changelog
# 4. Create GitHub release
# 5. Publish to crates.io (if configured)
```

## FAQ

### Why nextest instead of cargo test?

- 2-3x faster
- Better output formatting
- Per-test timeouts
- Retry flaky tests
- Industry standard (used by Google, Meta, AWS)

### Why sccache?

- Caches compiled Rust code
- 50-70% faster rebuilds
- Shares cache across jobs
- Designed for CI/CD

### Why cargo-llvm-cov?

- Accurate LLVM-based coverage
- Includes doctests
- Fast (uses nextest)
- Standard tool recommended by Rust community

### Why nightly Rust for coverage?

cargo-llvm-cov requires nightly for the `-C instrument-coverage` flag. This is only for the coverage job - all other jobs use stable Rust.

### Can I use the old `cargo test`?

Yes! Just run `cargo test` locally. CI uses nextest, but they're compatible.

### Why is the article_scraper test skipped?

Known segfault issue (RUSTSEC-2024-0436). Tracked in our docs. Will fix when upstream updates.

---

**Need Help?**

- Check the [CI Enhancement Plan](./GITHUB_ACTIONS_ENHANCEMENT.md)
- Read the [CI Comparison](./CI_COMPARISON.md)
- Ask in team chat or open an issue

**Last Updated**: 2025-10-08
