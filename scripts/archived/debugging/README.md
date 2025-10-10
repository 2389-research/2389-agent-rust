# Archived Debugging Scripts

This directory contains debugging scripts from historical issues that have been resolved.

These scripts are kept for reference but are not actively maintained.

## reproduce_segfault_linux.sh

**Date**: January 2025
**Issue**: Segfault in article_scraper tests on Linux (Ubuntu 24.04)
**Status**: RESOLVED

**Purpose**: Reproduce article_scraper segfault in Docker environment matching GitHub Actions

**What it did**:
- Ran tests in Ubuntu 24.04 Docker container
- Used strace, valgrind, and gdb to debug
- Identified libxml2 version issues

**Resolution**: Issue was resolved by updating dependencies and test isolation

**Historical Context**:
- Only reproduced on Linux (not macOS)
- Related to test harness and cleanup between tests
- Fixed in commits prior to v1.0

**To use this script** (for reference only):
```bash
cd /Users/clint/code/2389-agent-rust
./scripts/archived/debugging/reproduce_segfault_linux.sh
```

**Note**: This script is kept for historical reference. If similar issues arise,
it provides a template for reproducing and debugging platform-specific crashes.
