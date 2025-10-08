# Segfault Diagnosis Plan - article_scraper on Linux

## Problem Statement

The `article_scraper` tests pass successfully but the process segfaults (SIGSEGV signal 11) **during cleanup after all tests complete** on Linux CI (GitHub Actions ubuntu-latest), but works perfectly on macOS.

## Observed Symptoms

### ✅ Works on macOS (darwin)
```bash
$ cargo test test_article_extraction
test result: ok. 3 passed; 0 failed; 0 ignored
# Process exits cleanly
```

### ❌ Fails on Linux CI (ubuntu-latest)
```bash
$ cargo test test_article_extraction
test result: ok. 326 passed; 0 failed; 0 ignored
# Process crashes AFTER test completion
error: process didn't exit successfully: (signal: 11, SIGSEGV: invalid memory reference)
```

## Key Questions to Answer

1. **Why does it only fail on Linux and not macOS?**
   - Different allocators?
   - Different library versions?
   - Different cleanup order?

2. **Why does it fail during cleanup, not during tests?**
   - Destructor issue?
   - Double-free?
   - Use-after-free?

3. **Is it really article_scraper or something else?**
   - Could be any dependency
   - Could be test harness itself
   - Need to isolate

4. **What exactly in the dependency chain causes it?**
   - article_scraper → libxml?
   - article_scraper → image → rav1e → paste?
   - Something in tokio runtime shutdown?

## Investigation Plan

### Phase 1: Reproduce Locally in Linux

**Goal**: Get the exact same failure on local machine

**Steps**:
1. Use Docker to run Ubuntu container matching GitHub Actions
2. Run tests with same environment variables as CI
3. Capture the exact failure mode
4. Generate core dump for analysis

**Commands**:
```bash
# Use exact GitHub Actions Ubuntu image
# Note: ubuntu-latest = Ubuntu 24.04 as of Jan 2025
docker run -it --rm \
  -v $(pwd):/workspace \
  -w /workspace \
  ubuntu:24.04 \
  bash

# Or use our reproduction script (matches GitHub Actions exactly)
./scripts/reproduce_segfault_linux.sh

# To test Ubuntu 22.04 specifically:
UBUNTU_VERSION=22.04 ./scripts/reproduce_segfault_linux.sh

# Inside container:
apt-get update && apt-get install -y \
  curl build-essential pkg-config libssl-dev \
  libxml2-dev gdb valgrind

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# Enable core dumps
ulimit -c unlimited

# Run tests and capture core dump
RUST_BACKTRACE=full cargo test test_article_extraction 2>&1 | tee test-output.log

# If it crashes, analyze core dump
gdb target/debug/deps/<test-binary> core
```

### Phase 2: Get Detailed Backtrace

**Goal**: Understand exactly where the segfault occurs

**Steps**:
1. Run under gdb with full backtrace
2. Run under valgrind to detect memory errors
3. Check for double-free, use-after-free, etc.

**Commands**:
```bash
# Find the test binary
TEST_BINARY=$(cargo test --no-run test_article_extraction 2>&1 | grep -oP 'Running.*\K`[^`]+' | head -1 | tr -d '`')

# Run under gdb
gdb --args $TEST_BINARY test_article_extraction

# Inside gdb:
> run
> bt full  # When it crashes, get full backtrace
> info threads  # Check all threads
> thread apply all bt  # Backtrace for all threads

# Run under valgrind
valgrind --leak-check=full \
         --track-origins=yes \
         --show-leak-kinds=all \
         $TEST_BINARY test_article_extraction
```

### Phase 3: Isolate the Problematic Component

**Goal**: Determine which dependency causes the issue

**Steps**:
1. Create minimal test that only uses article_scraper
2. Test with article_scraper disabled
3. Test each article_scraper feature independently

**Test 1: Minimal reproduction**
```rust
// tests/minimal_article_scraper.rs
use article_scraper::Readability;

#[tokio::test]
async fn test_minimal_article_scraper() {
    let html = "<html><body><p>Test</p></body></html>";
    let result = Readability::extract(html, None).await;
    assert!(result.is_ok() || result.is_err()); // Don't care about result
}
```

**Test 2: Without libxml**
- article_scraper uses libxml2 (native library)
- This could have platform-specific issues

**Test 3: Without image processing**
- article_scraper uses `image` crate
- `image` → `ravif` → `rav1e` → `paste` (unmaintained)

### Phase 4: Check Library Versions

**Goal**: Determine if specific versions cause the issue

**Steps**:
1. Check libxml2 version on macOS vs Linux
2. Check if GitHub Actions has old/broken libxml2
3. Test with different article_scraper versions

**Commands**:
```bash
# On macOS
brew info libxml2

# On Linux
apt-cache policy libxml2-dev
ldconfig -p | grep libxml

# Try different article_scraper versions
# Current: 2.1.4
cargo update -p article_scraper --precise 2.1.3
cargo test test_article_extraction

cargo update -p article_scraper --precise 2.1.2
cargo test test_article_extraction
```

### Phase 5: Test Hypothesis - Double-Free in Cleanup

**Goal**: Determine if it's a double-free or use-after-free

**Theory**: article_scraper might have cleanup code that:
- Frees libxml2 resources twice
- Accesses freed memory
- Has race condition in Drop implementation

**Test**:
```rust
#[tokio::test]
async fn test_explicit_drop_order() {
    let html = "<html><body><p>Test</p></body></html>";

    {
        let result = Readability::extract(html, None).await.unwrap();
        // Explicitly drop before test ends
        drop(result);
    }

    // Force garbage collection
    std::mem::forget(vec![0u8; 1024 * 1024]);
}
```

### Phase 6: Check for Known Issues

**Goal**: See if this is a known problem

**Steps**:
1. Check article_scraper GitHub issues
2. Check libxml-rust GitHub issues
3. Check if others report Linux-specific crashes

**Search queries**:
- "article_scraper segfault"
- "article_scraper linux crash"
- "libxml-rust segfault"
- "rav1e segfault cleanup"
- "paste proc-macro segfault"

### Phase 7: Alternative Hypotheses

**Goal**: Consider other causes

**Hypothesis 1: Tokio Runtime Cleanup**
- Tests use tokio runtime
- Runtime shutdown might conflict with article_scraper cleanup
- Test: Run in blocking context instead

**Hypothesis 2: System Allocator Differences**
- macOS uses jemalloc by default
- Linux uses glibc malloc
- Test: Force same allocator on both

**Hypothesis 3: libxml2 Native Bindings**
- FFI to native C library
- Different behavior on different platforms
- Test: Check bindgen output

## Expected Outcomes

### If it's article_scraper:
- **Fix**: Update to newer version or patch
- **Workaround**: Use alternative implementation
- **Report**: File issue with backtrace

### If it's libxml2:
- **Fix**: Update libxml2 in CI
- **Workaround**: Use different XML parser
- **Report**: Document platform requirement

### If it's paste/rav1e:
- **Fix**: Wait for paste fork (pastey)
- **Workaround**: Exclude image processing
- **Report**: Already known (RUSTSEC-2024-0436)

### If it's test harness:
- **Fix**: Change test structure
- **Workaround**: Run tests differently
- **Report**: Cargo/Rust issue

## Success Criteria

We will consider this investigation complete when we can:

1. ✅ Reproduce the segfault locally in Docker Linux
2. ✅ Get a full backtrace showing exact crash location
3. ✅ Identify the specific library/function causing the crash
4. ✅ Understand WHY it works on macOS but not Linux
5. ✅ Have a proper fix (not just skipping tests)

## Timeline

- **Phase 1-2**: 1-2 hours (reproduce and get backtrace)
- **Phase 3-4**: 1-2 hours (isolate component)
- **Phase 5-7**: 1-2 hours (test hypotheses)
- **Total**: 3-6 hours

## Notes

- All tests PASS - this is not a test failure
- Crash happens AFTER tests complete
- This suggests destructor/cleanup issue
- macOS vs Linux difference suggests platform-specific code path

## Next Steps

1. Start with Phase 1 - reproduce in Docker
2. Get backtrace with gdb
3. Based on findings, proceed to isolation or version testing

---

**Status**: Ready to Execute
**Owner**: MR BEEF
**Created**: 2025-10-08
**Priority**: HIGH - blocking CI from running all tests
