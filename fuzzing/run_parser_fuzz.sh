#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   ./fuzzing/run_parser_fuzz.sh                  # run all targets, 60s each
#   ./fuzzing/run_parser_fuzz.sh fuzz_detect_start # run one target
#   FUZZ_TIMEOUT=300 ./fuzzing/run_parser_fuzz.sh  # 5 min per target
#
# Bug-class flags (all configurable via env):
#   FUZZ_TIMEOUT_PER_INPUT=10  # seconds per single input (catches hangs/DoS)
#   FUZZ_RSS_LIMIT=2048        # MB memory limit (catches quadratic blowups)
#   FUZZ_MAX_LEN=65536         # max input size in bytes
#
# Overflow checking (catches integer overflow bugs in release mode):
#   FUZZ_OVERFLOW_CHECKS=1 ./fuzzing/run_parser_fuzz.sh

FUZZ_TIMEOUT="${FUZZ_TIMEOUT:-60}"
FUZZ_TIMEOUT_PER_INPUT="${FUZZ_TIMEOUT_PER_INPUT:-10}"
FUZZ_RSS_LIMIT="${FUZZ_RSS_LIMIT:-2048}"
FUZZ_MAX_LEN="${FUZZ_MAX_LEN:-65536}"
FUZZ_OVERFLOW_CHECKS="${FUZZ_OVERFLOW_CHECKS:-0}"
FUZZ_DICT="${FUZZ_DICT:-}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FUZZ_ENV="$SCRIPT_DIR/.fuzz-env"

export RUSTUP_HOME="$FUZZ_ENV/rustup"
export CARGO_HOME="$FUZZ_ENV/cargo"
export PATH="$CARGO_HOME/bin:$PATH"

ALL_TARGETS=(
    fuzz_tool_call_parsers
    fuzz_parser_configs
    fuzz_reasoning_parsers
    fuzz_streaming_reasoning
    fuzz_detect_start
    fuzz_end_positions
    fuzz_deepseek_parsers
    fuzz_structured_configs
    fuzz_nested_and_large
    fuzz_invariants
    fuzz_differential
    fuzz_redos
    fuzz_with_tools
)

# Install toolchain if needed
if [ ! -f "$CARGO_HOME/bin/rustup" ]; then
    echo "=== Installing rustup (isolated)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain nightly
fi
if ! rustup toolchain list | grep -q nightly; then
    rustup install nightly
fi
if ! command -v cargo-fuzz &>/dev/null; then
    cargo +nightly install cargo-fuzz
fi

# Enable overflow checks if requested (catches integer wrapping bugs)
if [ "$FUZZ_OVERFLOW_CHECKS" = "1" ]; then
    export RUSTFLAGS="${RUSTFLAGS:-} -C overflow-checks=yes"
    echo "=== Overflow checks ENABLED"
fi

# Select targets
if [ $# -gt 0 ]; then
    TARGETS=("$@")
else
    TARGETS=("${ALL_TARGETS[@]}")
fi

cd "$REPO_ROOT/lib/parsers"

# Copy seed corpus to target-specific corpus dirs on first run
SEED_DIR="fuzz/seeds"
if [ -d "$SEED_DIR" ]; then
    for target in "${TARGETS[@]}"; do
        corpus_dir="fuzz/corpus/$target"
        mkdir -p "$corpus_dir"
        if [ "$(find "$corpus_dir" -maxdepth 1 -type f 2>/dev/null | wc -l | tr -d ' ')" -eq 0 ]; then
            cp "$SEED_DIR"/* "$corpus_dir"/ 2>/dev/null || true
        fi
    done
fi

echo "=== Config: timeout=${FUZZ_TIMEOUT}s/target, ${FUZZ_TIMEOUT_PER_INPUT}s/input, rss=${FUZZ_RSS_LIMIT}MB, max_len=${FUZZ_MAX_LEN}"
echo "=== Targets: ${#TARGETS[@]}"

CRASHES_FOUND=0
for target in "${TARGETS[@]}"; do
    echo ""
    echo "=== [$target] running for ${FUZZ_TIMEOUT}s..."
    DICT_ARG=()
    if [ -n "$FUZZ_DICT" ]; then
        DICT_ARG=("-dict=$FUZZ_DICT")
    fi

    cargo +nightly fuzz run "$target" -- \
        -max_total_time="$FUZZ_TIMEOUT" \
        -timeout="$FUZZ_TIMEOUT_PER_INPUT" \
        -rss_limit_mb="$FUZZ_RSS_LIMIT" \
        -max_len="$FUZZ_MAX_LEN" \
        "${DICT_ARG[@]}" \
        2>&1 || true

    artifact_dir="fuzz/artifacts/$target"
    count=$(find "$artifact_dir" -type f 2>/dev/null | wc -l | tr -d ' ')
    if [ "$count" -gt 0 ]; then
        echo "=== [$target] FOUND $count crash(es) in $artifact_dir/"
        CRASHES_FOUND=$((CRASHES_FOUND + count))
    else
        echo "=== [$target] no crashes"
    fi
done

echo ""
echo "=== Summary: $CRASHES_FOUND total crash(es) across ${#TARGETS[@]} targets"
echo "=== Artifacts: lib/parsers/fuzz/artifacts/<target>/"
echo "=== Corpus:    lib/parsers/fuzz/corpus/<target>/"
