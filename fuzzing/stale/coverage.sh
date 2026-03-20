#!/usr/bin/env bash
set -euo pipefail

# Generate HTML coverage reports for fuzz targets.
# Usage:
#   ./fuzzing/coverage.sh                        # all crates, all targets
#   ./fuzzing/coverage.sh --crate parsers        # one crate
#   ./fuzzing/coverage.sh --target fuzz_invariants  # one target

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FUZZ_ENV="$SCRIPT_DIR/.fuzz-env"
COVERAGE_DIR="$SCRIPT_DIR/coverage-report"
FUZZ_CRATE=""
FUZZ_TARGET=""

export RUSTUP_HOME="$FUZZ_ENV/rustup"
export CARGO_HOME="$FUZZ_ENV/cargo"
export PATH="$CARGO_HOME/bin:$PATH"

while [[ $# -gt 0 ]]; do
    case $1 in
        --crate) FUZZ_CRATE="$2"; shift 2 ;;
        --target) FUZZ_TARGET="$2"; shift 2 ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

# Ensure toolchain
if [ ! -f "$CARGO_HOME/bin/rustup" ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain nightly
fi
rustup +nightly component add llvm-tools-preview 2>/dev/null || true
command -v cargo-fuzz &>/dev/null || cargo +nightly install cargo-fuzz

mkdir -p "$COVERAGE_DIR"

# Auto-discover and iterate crates
find "$REPO_ROOT/lib" -path "*/fuzz/Cargo.toml" -maxdepth 3 | sort | while read -r toml; do
    crate_dir="$(dirname "$(dirname "$toml")")"
    crate_name="$(basename "$crate_dir")"

    [ -n "$FUZZ_CRATE" ] && [ "$crate_name" != "$FUZZ_CRATE" ] && continue

    cd "$crate_dir"
    echo "=== Crate: $crate_name"

    for target_file in fuzz/fuzz_targets/*.rs; do
        [ -f "$target_file" ] || continue
        target="$(basename "$target_file" .rs)"
        [ -n "$FUZZ_TARGET" ] && [ "$target" != "$FUZZ_TARGET" ] && continue

        corpus_dir="fuzz/corpus/$target"
        if [ ! -d "$corpus_dir" ] || [ "$(find "$corpus_dir" -maxdepth 1 -type f 2>/dev/null | wc -l | tr -d ' ')" -eq 0 ]; then
            echo "--- [$target] No corpus — skipping"
            continue
        fi

        echo "--- [$target] Generating coverage..."
        cargo +nightly fuzz coverage "$target" 2>&1 || { echo "--- [$target] failed"; continue; }

        COVERAGE_BIN=$(find "target" -path "*/coverage/*/$target" -type f -executable 2>/dev/null | head -1)
        PROFDATA=$(find "fuzz/coverage/$target" -name "*.profdata" 2>/dev/null | head -1)
        LLVM_COV=$(find "$RUSTUP_HOME" -name "llvm-cov" -type f 2>/dev/null | head -1)

        if [ -z "$COVERAGE_BIN" ] || [ -z "$PROFDATA" ] || [ -z "$LLVM_COV" ]; then
            echo "--- [$target] Missing coverage binary/profdata/llvm-cov"
            continue
        fi

        TARGET_DIR="$COVERAGE_DIR/$crate_name/$target"
        mkdir -p "$TARGET_DIR"
        "$LLVM_COV" show "$COVERAGE_BIN" \
            --instr-profile="$PROFDATA" \
            --format=html \
            --output-dir="$TARGET_DIR" \
            --ignore-filename-regex='/.cargo/|/rustc/|fuzz_targets/' 2>&1 || true
        echo "--- [$target] Report: $TARGET_DIR/index.html"
    done
done

echo ""
echo "=== Coverage reports: $COVERAGE_DIR/"
