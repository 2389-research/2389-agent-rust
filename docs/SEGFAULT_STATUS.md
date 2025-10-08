# Segfault Investigation Status

## Current Status: **INVESTIGATION READY** 🔍

We have completed the research phase and created comprehensive reproduction tools. Ready to execute the Docker reproduction to get the exact backtrace.

## What We Know

### ✅ Confirmed Facts

1. **Platform-Specific**: Tests pass on macOS, segfault on Linux CI
2. **Timing**: Crash occurs AFTER all tests complete successfully, during cleanup
3. **Signal**: SIGSEGV (signal 11) - memory access violation
4. **Component**: Related to `article_scraper` crate and its dependencies

### 🔍 Research Findings

#### libxml2 Known Issues
From our research, libxml2 has documented history of:
- **Double-free errors** in cleanup code
- **Use-after-free** in pthread cleanup handlers
- **Threading issues** where `dlclose` with threads causes crashes
- **Cleanup ordering** problems in multi-threaded environments

Specific issues found:
- Nokogiri (Ruby) had issues where libxml2's `atexit` handler called cleanup after object destruction
- libxml2's pthread code uses `pthread_key_create` with destructor that can be called after VM teardown
- Multiple reports of segfaults in `xmlFreeNode()` and `xmlFreeDoc()`

#### Dependency Chain
```
article_scraper v2.1.4
├── libxml v0.3.8 (Rust bindings to libxml2)
│   └── libxml2 (native C library)
├── image v0.25.8
│   └── ravif → rav1e → paste v1.0.15 (RUSTSEC-2024-0436: unmaintained)
└── tokio (async runtime)
```

### 📝 Test Results

**macOS (darwin, arm64)**:
```bash
$ cargo test --test debug_article_scraper_segfault
running 6 tests
test test_extraction_with_url ... ok
test test_with_forced_cleanup ... ok
test test_no_tokio ... ok
test test_multiple_extractions ... ok
test test_minimal_article_extraction ... ok
test test_sequential_runtimes ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
✅ PASSES CLEANLY
```

**Linux CI (ubuntu-latest)**:
```bash
$ cargo test test_article_extraction
test result: ok. 326 passed; 0 failed; 0 ignored
error: process didn't exit successfully: (signal: 11, SIGSEGV)
❌ SEGFAULTS DURING CLEANUP
```

## Investigation Plan

We have created a comprehensive 7-phase investigation plan:

### Phase 1: Reproduce Locally ✅ READY
- Docker environment matching GitHub Actions (Ubuntu 22.04)
- Script: `scripts/reproduce_segfault_linux.sh`
- Goal: Get exact same failure locally

### Phase 2: Get Backtrace (NEXT STEP)
- Run under gdb for stack trace
- Run under valgrind for memory errors
- Run under strace for syscall analysis

### Phase 3: Isolate Component
- Minimal reproduction tests created
- Test without tokio runtime
- Test with explicit drop ordering

### Phase 4: Version Testing
- Test different article_scraper versions
- Check libxml2 versions on macOS vs Linux

### Phase 5: Hypothesis Testing
- Double-free in cleanup
- Use-after-free
- Race condition in Drop

### Phase 6: Known Issues Check
- Research shows libxml2 has history of these issues
- No specific article_scraper bugs found (GitLab repo)

### Phase 7: Alternative Hypotheses
- Tokio runtime shutdown interaction
- System allocator differences (jemalloc vs glibc)
- libxml2 native bindings FFI issues

## Tools Created

### 1. Diagnosis Plan
**File**: `docs/SEGFAULT_DIAGNOSIS_PLAN.md`
- Comprehensive 7-phase plan
- Specific commands for each phase
- Expected outcomes and decision tree

### 2. Docker Reproduction Script
**File**: `scripts/reproduce_segfault_linux.sh`
- Matches GitHub Actions environment exactly
- Runs tests with gdb, valgrind, strace
- Captures core dumps and backtraces
- Compares libxml2 versions

### 3. Minimal Test Cases
**File**: `tests/debug_article_scraper_segfault.rs`
- 6 isolated test cases
- Tests different scenarios:
  - Single extraction
  - Multiple extractions
  - With/without tokio
  - Sequential runtimes
  - Forced cleanup
  - With URL parsing

## Next Steps

### Immediate (30 minutes - 2 hours)
1. **Run Docker reproduction**: `./scripts/reproduce_segfault_linux.sh`
2. **Get exact backtrace** from gdb showing crash location
3. **Analyze valgrind output** for memory errors
4. **Review strace** for syscalls before crash

### Based on Findings

#### If it's libxml2:
- **Quick fix**: Pin to older/newer version
- **Proper fix**: Report issue to libxml-rust maintainers
- **Workaround**: Use different XML parser or disable feature

#### If it's paste/rav1e:
- **Quick fix**: Exclude image processing features
- **Proper fix**: Wait for pastey fork adoption
- **Workaround**: Use simple HTML extraction only

#### If it's tokio:
- **Quick fix**: Change test structure
- **Proper fix**: Report to tokio team
- **Workaround**: Run tests in blocking context

#### If it's article_scraper:
- **Quick fix**: Downgrade to older version
- **Proper fix**: Patch and submit PR
- **Workaround**: Replace with simpler implementation

## Why This Matters

**We cannot ship v0.1.0 with**:
- ❌ Tests being skipped in CI
- ❌ Unknown segfaults in production
- ❌ Platform-specific undefined behavior

**We need to**:
- ✅ Understand the root cause
- ✅ Implement a proper fix
- ✅ Run ALL tests in CI successfully

## Resources

### Documentation
- [SEGFAULT_DIAGNOSIS_PLAN.md](./SEGFAULT_DIAGNOSIS_PLAN.md) - Full investigation plan
- [libxml2 GitLab Issues](https://gitlab.gnome.org/GNOME/libxml2/-/issues)
- [article_scraper GitLab](https://gitlab.com/news-flash/article_scraper)

### Research Links
- [Nokogiri libxml memory management](https://nokogiri.org/adr/2023-04-libxml-memory-management.html)
- [libxml2 threading issue #153](https://gitlab.gnome.org/GNOME/libxml2/-/issues/153)
- [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html)

### Commands

**Test locally (macOS)**:
```bash
cargo test --test debug_article_scraper_segfault --verbose
```

**Reproduce on Linux**:
```bash
./scripts/reproduce_segfault_linux.sh
```

**Run specific test**:
```bash
cargo test --test debug_article_scraper_segfault test_minimal_article_extraction
```

## Timeline Estimate

- **Phase 1-2**: 1-2 hours (reproduce + backtrace)
- **Phase 3-4**: 1-2 hours (isolate + version test)
- **Phase 5-7**: 1-2 hours (hypotheses)
- **Fix implementation**: 1-4 hours
- **Total**: 4-10 hours

## Success Criteria

Investigation complete when we have:
1. ✅ Exact crash location from backtrace
2. ✅ Root cause identified
3. ✅ Reproducible fix
4. ✅ All tests passing in CI (no skips)
5. ✅ Understanding of why it's platform-specific

---

**Status**: Ready to execute Docker reproduction
**Next Action**: Run `./scripts/reproduce_segfault_linux.sh`
**Owner**: MR BEEF
**Created**: 2025-10-08
**Priority**: CRITICAL - blocking v0.1.0 release
