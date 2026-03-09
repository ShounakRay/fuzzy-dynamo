#!/usr/bin/env bash
set -euo pipefail

# Unified fuzzing runner. Auto-discovers all fuzz crates under lib/.
# Usage:
#   ./fuzzing/run.sh                              # all crates, 60s/target
#   ./fuzzing/run.sh --crate parsers              # one crate
#   ./fuzzing/run.sh --target fuzz_invariants     # one target
#   FUZZ_TIMEOUT=300 ./fuzzing/run.sh             # 5 min/target

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

if [ -f "/opt/homebrew/anaconda3/bin/protoc" ]; then
    export PROTOC="/opt/homebrew/anaconda3/bin/protoc"
    export PATH="/opt/homebrew/anaconda3/bin:$PATH"
elif command -v protoc &>/dev/null; then
    export PROTOC="$(command -v protoc)"
fi

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
rustup toolchain list | grep -q nightly || rustup install nightly
command -v cargo-fuzz &>/dev/null || cargo +nightly install cargo-fuzz

[ "$FUZZ_OVERFLOW_CHECKS" = "1" ] && export RUSTFLAGS="${RUSTFLAGS:-} -C overflow-checks=yes"

echo "=== Fuzzing: timeout=${FUZZ_TIMEOUT}s, rss=${FUZZ_RSS_LIMIT}MB, max_len=${FUZZ_MAX_LEN}"

TOTAL_CRASHES=0
TOTAL_TARGETS=0

find "$REPO_ROOT/lib" -path "*/fuzz/Cargo.toml" -maxdepth 3 | sort | while read -r toml; do
    crate_dir="$(dirname "$(dirname "$toml")")"
    crate_name="$(basename "$crate_dir")"
    [ -n "$FUZZ_CRATE" ] && [ "$crate_name" != "$FUZZ_CRATE" ] && continue

    echo ""
    echo "=== Crate: $crate_name"
    cd "$crate_dir"

    dict=$(find fuzz -name "*.dict" -type f 2>/dev/null | head -1)
    seeds_dir="fuzz/seeds"

    for target_file in fuzz/fuzz_targets/*.rs; do
        [ -f "$target_file" ] || continue
        target="$(basename "$target_file" .rs)"
        [ -n "$FUZZ_TARGET" ] && [ "$target" != "$FUZZ_TARGET" ] && continue

        TOTAL_TARGETS=$((TOTAL_TARGETS + 1))
        corpus_dir="fuzz/corpus/$target"
        mkdir -p "$corpus_dir"

        # Seed corpus on first run
        if [ -d "$seeds_dir" ] && [ "$(find "$corpus_dir" -maxdepth 1 -type f 2>/dev/null | wc -l | tr -d ' ')" -eq 0 ]; then
            cp "$seeds_dir"/* "$corpus_dir"/ 2>/dev/null || true
        fi

        DICT_ARG=()
        [ -n "$dict" ] && DICT_ARG=("-dict=$dict")

        echo "--- [$crate_name/$target] ${FUZZ_TIMEOUT}s..."
        cargo +nightly fuzz run "$target" -- \
            -max_total_time="$FUZZ_TIMEOUT" \
            -timeout="$FUZZ_TIMEOUT_PER_INPUT" \
            -rss_limit_mb="$FUZZ_RSS_LIMIT" \
            -max_len="$FUZZ_MAX_LEN" \
            "${DICT_ARG[@]}" 2>&1 || true

        artifact_dir="fuzz/artifacts/$target"
        count=$(find "$artifact_dir" -type f 2>/dev/null | wc -l | tr -d ' ')
        if [ "$count" -gt 0 ]; then
            echo "--- [$crate_name/$target] $count crash(es)"
            TOTAL_CRASHES=$((TOTAL_CRASHES + count))
        fi
    done
done

echo ""
echo "=== Done: $TOTAL_TARGETS targets, $TOTAL_CRASHES crashes"
