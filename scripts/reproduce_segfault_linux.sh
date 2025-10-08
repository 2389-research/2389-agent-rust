#!/bin/bash
# Reproduce article_scraper segfault on Linux
#
# This script runs our tests in a Docker container matching GitHub Actions
# environment to reproduce the segfault locally.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Article Scraper Segfault Reproduction ==="
echo "Project root: $PROJECT_ROOT"
echo ""

# Use the same Ubuntu version as GitHub Actions
UBUNTU_VERSION="22.04"
RUST_VERSION="stable"

echo "Building Docker image with Ubuntu $UBUNTU_VERSION..."

docker build -t article-scraper-debug -f - "$PROJECT_ROOT" <<'EOF'
FROM ubuntu:22.04

# Prevent interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    libxml2-dev \
    gdb \
    valgrind \
    strace \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# Enable core dumps
RUN ulimit -c unlimited || true

WORKDIR /workspace

# Copy project files
COPY . .

# Pre-build to cache dependencies
RUN cargo build --tests 2>&1 | head -20 || true

CMD ["/bin/bash"]
EOF

echo ""
echo "=== Running tests in Docker ==="
echo ""

# Run tests with various debugging options
docker run --rm \
    -v "$PROJECT_ROOT":/workspace \
    -w /workspace \
    --cap-add=SYS_PTRACE \
    --security-opt seccomp=unconfined \
    article-scraper-debug \
    bash -c '
        echo "=== Test 1: Run minimal article_scraper test ==="
        echo "This should reproduce the segfault..."
        echo ""

        RUST_BACKTRACE=full cargo test --test debug_article_scraper_segfault --verbose 2>&1 | tee /tmp/test-output.log
        EXIT_CODE=${PIPESTATUS[0]}

        echo ""
        echo "Exit code: $EXIT_CODE"

        if [ $EXIT_CODE -ne 0 ]; then
            echo ""
            echo "=== SEGFAULT REPRODUCED! ==="
            echo ""
            echo "Checking for core dump..."
            find / -name "core*" 2>/dev/null | head -5

            echo ""
            echo "=== Test 2: Run with strace to see system calls ==="
            echo ""
            TEST_BINARY=$(cargo test --test debug_article_scraper_segfault --no-run --message-format=json 2>/dev/null | jq -r "select(.reason == \"compiler-artifact\") | select(.target.name == \"debug_article_scraper_segfault\") | .filenames[0]" | head -1)
            echo "Test binary: $TEST_BINARY"

            if [ -n "$TEST_BINARY" ] && [ -f "$TEST_BINARY" ]; then
                echo "Running with strace..."
                strace -o /tmp/strace.log "$TEST_BINARY" --test-threads=1 test_minimal_article_extraction 2>&1 || true

                echo ""
                echo "Last 50 lines of strace output:"
                tail -50 /tmp/strace.log

                echo ""
                echo "Looking for memory-related syscalls before crash:"
                grep -E "(munmap|free|brk)" /tmp/strace.log | tail -20
            fi

            echo ""
            echo "=== Test 3: Run with valgrind ==="
            echo ""
            if [ -n "$TEST_BINARY" ] && [ -f "$TEST_BINARY" ]; then
                valgrind \
                    --leak-check=full \
                    --track-origins=yes \
                    --show-leak-kinds=all \
                    --log-file=/tmp/valgrind.log \
                    "$TEST_BINARY" --test-threads=1 test_minimal_article_extraction 2>&1 || true

                echo ""
                echo "Valgrind output:"
                cat /tmp/valgrind.log
            fi

            echo ""
            echo "=== Test 4: Run with gdb to get backtrace ==="
            echo ""
            if [ -n "$TEST_BINARY" ] && [ -f "$TEST_BINARY" ]; then
                gdb -batch \
                    -ex "run --test-threads=1 test_minimal_article_extraction" \
                    -ex "bt" \
                    -ex "info threads" \
                    -ex "thread apply all bt" \
                    -ex "quit" \
                    "$TEST_BINARY" 2>&1 | tee /tmp/gdb-backtrace.log
            fi
        else
            echo ""
            echo "=== Tests passed without segfault ==="
            echo "This suggests the issue might be test-harness specific or related to multiple tests running."
            echo ""
            echo "Trying to run ALL article extraction tests..."
            cargo test test_article_extraction --verbose 2>&1 | tee /tmp/all-tests.log
            ALL_EXIT_CODE=${PIPESTATUS[0]}

            if [ $ALL_EXIT_CODE -ne 0 ]; then
                echo ""
                echo "=== SEGFAULT with all tests! ==="
                echo "This suggests the issue is related to test cleanup or multiple test execution."
            fi
        fi

        echo ""
        echo "=== Checking libxml2 version ==="
        pkg-config --modversion libxml-2.0

        echo ""
        echo "=== Checking system info ==="
        uname -a
        cat /etc/os-release | grep -E "(NAME|VERSION)"

        echo ""
        echo "=== Dependency tree for article_scraper ==="
        cargo tree -p article_scraper | head -50

        echo ""
        echo "=== Done ==="
    '

echo ""
echo "=== Analysis Complete ==="
echo ""
echo "Check the output above for:"
echo "  - Exact crash location from gdb backtrace"
echo "  - Memory errors from valgrind"
echo "  - System calls before crash from strace"
echo "  - libxml2 version differences"
echo ""
echo "Compare with macOS:"
echo "  - Run: cargo test --test debug_article_scraper_segfault"
echo "  - Check if it passes without segfault"
echo ""
