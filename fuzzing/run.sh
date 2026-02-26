#!/usr/bin/env bash
set -euo pipefail

# Unified fuzzing runner for all fuzz crates in the repository.
#
# Usage:
#   ./fuzzing/run.sh                              # all crates, all targets, 60s each
#   ./fuzzing/run.sh --crate parsers              # one crate only
#   ./fuzzing/run.sh --target fuzz_invariants     # one target (auto-detects crate)
#   FUZZ_TIMEOUT=300 ./fuzzing/run.sh             # 5 min per target
#
# Environment variables:
#   FUZZ_TIMEOUT=60            # seconds per target (default 60)
#   FUZZ_TIMEOUT_PER_INPUT=10  # seconds per input (catches hangs)
#   FUZZ_RSS_LIMIT=2048        # MB memory limit (catches OOM/quadratic)
#   FUZZ_MAX_LEN=65536         # max input size in bytes
#   FUZZ_OVERFLOW_CHECKS=0     # set to 1 to enable integer overflow detection
#   FUZZ_CRATE=                # filter to specific crate (parsers|kv-router|tokens|runtime)
#   FUZZ_TARGET=               # filter to specific target name

FUZZ_TIMEOUT="${FUZZ_TIMEOUT:-60}"
FUZZ_TIMEOUT_PER_INPUT="${FUZZ_TIMEOUT_PER_INPUT:-10}"
FUZZ_RSS_LIMIT="${FUZZ_RSS_LIMIT:-2048}"
FUZZ_MAX_LEN="${FUZZ_MAX_LEN:-65536}"
FUZZ_OVERFLOW_CHECKS="${FUZZ_OVERFLOW_CHECKS:-0}"
FUZZ_CRATE="${FUZZ_CRATE:-}"
FUZZ_TARGET="${FUZZ_TARGET:-}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FUZZ_ENV="$SCRIPT_DIR/.fuzz-env"

export RUSTUP_HOME="$FUZZ_ENV/rustup"
export CARGO_HOME="$FUZZ_ENV/cargo"
export PATH="$CARGO_HOME/bin:$PATH"

# protoc for crates that need it (etcd-client)
if [ -f "/opt/homebrew/anaconda3/bin/protoc" ]; then
    export PROTOC="/opt/homebrew/anaconda3/bin/protoc"
    export PATH="/opt/homebrew/anaconda3/bin:$PATH"
elif command -v protoc &>/dev/null; then
    export PROTOC="$(command -v protoc)"
fi

# Parse CLI arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --crate) FUZZ_CRATE="$2"; shift 2 ;;
        --target) FUZZ_TARGET="$2"; shift 2 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

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

# Enable overflow checks if requested
if [ "$FUZZ_OVERFLOW_CHECKS" = "1" ]; then
    export RUSTFLAGS="${RUSTFLAGS:-} -C overflow-checks=yes"
    echo "=== Overflow checks ENABLED"
fi

# Auto-discover fuzz crates
discover_crates() {
    find "$REPO_ROOT/lib" -path "*/fuzz/Cargo.toml" -maxdepth 3 | sort | while read -r toml; do
        crate_dir="$(dirname "$(dirname "$toml")")"
        crate_name="$(basename "$crate_dir")"
        echo "$crate_name:$crate_dir"
    done
}

# Get targets for a fuzz crate
list_targets() {
    local crate_dir="$1"
    find "$crate_dir/fuzz/fuzz_targets" -name "*.rs" -type f 2>/dev/null | while read -r f; do
        basename "$f" .rs
    done | sort
}

# Find dictionary for a crate
find_dict() {
    local crate_dir="$1"
    find "$crate_dir/fuzz" -name "*.dict" -type f 2>/dev/null | head -1
}

# Find seeds directory for a crate
find_seeds() {
    local crate_dir="$1"
    local seeds_dir="$crate_dir/fuzz/seeds"
    if [ -d "$seeds_dir" ]; then
        echo "$seeds_dir"
    fi
}

echo "=== Unified Fuzzing Runner"
echo "=== Config: timeout=${FUZZ_TIMEOUT}s/target, ${FUZZ_TIMEOUT_PER_INPUT}s/input, rss=${FUZZ_RSS_LIMIT}MB, max_len=${FUZZ_MAX_LEN}"
echo ""

TOTAL_CRASHES=0
TOTAL_TARGETS=0
CRATES_RUN=0

while IFS=: read -r crate_name crate_dir; do
    # Apply crate filter
    if [ -n "$FUZZ_CRATE" ] && [ "$crate_name" != "$FUZZ_CRATE" ]; then
        continue
    fi

    fuzz_dir="$crate_dir/fuzz"
    dict=$(find_dict "$crate_dir")
    seeds=$(find_seeds "$crate_dir")

    echo "=== Crate: $crate_name ($crate_dir)"
    CRATES_RUN=$((CRATES_RUN + 1))

    cd "$crate_dir"

    targets=$(list_targets "$crate_dir")
    crate_crashes=0

    for target in $targets; do
        # Apply target filter
        if [ -n "$FUZZ_TARGET" ] && [ "$target" != "$FUZZ_TARGET" ]; then
            continue
        fi

        TOTAL_TARGETS=$((TOTAL_TARGETS + 1))

        # Seed corpus directory
        corpus_dir="fuzz/corpus/$target"
        mkdir -p "$corpus_dir"

        # Copy seeds to corpus on first run
        if [ -n "$seeds" ] && [ "$(find "$corpus_dir" -maxdepth 1 -type f 2>/dev/null | wc -l | tr -d ' ')" -eq 0 ]; then
            cp "$seeds"/* "$corpus_dir"/ 2>/dev/null || true
        fi

        # Build dict argument
        DICT_ARG=()
        if [ -n "$dict" ]; then
            DICT_ARG=("-dict=$dict")
        fi

        echo ""
        echo "--- [$crate_name/$target] running for ${FUZZ_TIMEOUT}s..."

        cargo +nightly fuzz run "$target" -- \
            -max_total_time="$FUZZ_TIMEOUT" \
            -timeout="$FUZZ_TIMEOUT_PER_INPUT" \
            -rss_limit_mb="$FUZZ_RSS_LIMIT" \
            -max_len="$FUZZ_MAX_LEN" \
            "${DICT_ARG[@]}" \
            2>&1 || true

        # Count crashes
        artifact_dir="fuzz/artifacts/$target"
        count=$(find "$artifact_dir" -type f 2>/dev/null | wc -l | tr -d ' ')
        if [ "$count" -gt 0 ]; then
            echo "--- [$crate_name/$target] FOUND $count crash(es) in $artifact_dir/"
            crate_crashes=$((crate_crashes + count))
        else
            echo "--- [$crate_name/$target] no crashes"
        fi
    done

    TOTAL_CRASHES=$((TOTAL_CRASHES + crate_crashes))
    echo ""
    echo "=== [$crate_name] $crate_crashes crash(es) across targets"

done < <(discover_crates)

echo ""
echo "============================================"
echo "=== Summary"
echo "=== Crates: $CRATES_RUN"
echo "=== Targets: $TOTAL_TARGETS"
echo "=== Total crashes: $TOTAL_CRASHES"
echo "============================================"
