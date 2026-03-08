#!/usr/bin/env bash
# Generate coverage reports for fuzz targets.
#
# Usage:
#   ./scripts/fuzz-coverage.sh                  # all crates
#   ./scripts/fuzz-coverage.sh parsers           # single crate
#   ./scripts/fuzz-coverage.sh kv-router fuzz_radix_tree_events  # single target
#
# Prerequisites:
#   rustup component add llvm-tools-preview --toolchain nightly

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COVERAGE_DIR="$REPO_ROOT/coverage"

# Locate llvm tools from the nightly toolchain
find_llvm_tool() {
    local tool="$1"
    # Try PATH first
    if command -v "$tool" &>/dev/null; then
        command -v "$tool"
        return
    fi
    # Search in rustup toolchain
    local sysroot
    sysroot="$(rustc +nightly --print sysroot 2>/dev/null)" || true
    if [[ -n "$sysroot" ]]; then
        local found
        found="$(find "$sysroot" -name "$tool" -type f 2>/dev/null | head -1)"
        if [[ -n "$found" ]]; then
            echo "$found"
            return
        fi
    fi
    echo >&2 "ERROR: $tool not found. Run: rustup component add llvm-tools-preview --toolchain nightly"
    exit 1
}

LLVM_PROFDATA="$(find_llvm_tool llvm-profdata)"
LLVM_COV="$(find_llvm_tool llvm-cov)"

echo "Using llvm-profdata: $LLVM_PROFDATA"
echo "Using llvm-cov:      $LLVM_COV"

# Define crates and their targets
declare -A CRATE_DIRS=(
    [parsers]="lib/parsers/fuzz"
    [kv-router]="lib/kv-router/fuzz"
    [tokens]="lib/tokens/fuzz"
    [runtime]="lib/runtime/fuzz"
)

get_targets() {
    local crate_dir="$1"
    cd "$REPO_ROOT/$crate_dir"
    ~/.cargo/bin/cargo +nightly fuzz list 2>/dev/null
}

run_coverage() {
    local crate_name="$1"
    local target="$2"
    local crate_dir="${CRATE_DIRS[$crate_name]}"
    local fuzz_dir="$REPO_ROOT/$crate_dir"
    local corpus_dir="$fuzz_dir/corpus/$target"
    local out_dir="$COVERAGE_DIR/$crate_name/$target"

    if [[ ! -d "$corpus_dir" ]] || [[ -z "$(ls -A "$corpus_dir" 2>/dev/null)" ]]; then
        echo "  SKIP $crate_name/$target (no corpus)"
        return
    fi

    echo "  COVERAGE $crate_name/$target"

    cd "$fuzz_dir"

    # Generate profraw files
    ~/.cargo/bin/cargo +nightly fuzz coverage "$target" "$corpus_dir" 2>/dev/null || {
        echo "  WARN: cargo fuzz coverage failed for $target"
        return
    }

    # Find profraw files
    local profraw_dir="$fuzz_dir/coverage/$target"
    if [[ ! -d "$profraw_dir" ]]; then
        echo "  WARN: no profraw directory for $target"
        return
    fi

    local profraw_files
    profraw_files="$(find "$profraw_dir" -name '*.profraw' 2>/dev/null)"
    if [[ -z "$profraw_files" ]]; then
        echo "  WARN: no profraw files for $target"
        return
    fi

    # Merge profiles
    mkdir -p "$out_dir"
    "$LLVM_PROFDATA" merge -sparse $profraw_files -o "$out_dir/merged.profdata"

    # Find the fuzz binary
    local bin_path
    bin_path="$(find "$fuzz_dir/target" -name "$target" -type f -path '*/release/*' 2>/dev/null | head -1)"
    if [[ -z "$bin_path" ]]; then
        echo "  WARN: binary not found for $target"
        return
    fi

    # Generate HTML report
    "$LLVM_COV" show "$bin_path" \
        --instr-profile="$out_dir/merged.profdata" \
        --format=html \
        --output-dir="$out_dir/html" \
        --ignore-filename-regex='\.cargo/registry|rustc|/fuzz_targets/' 2>/dev/null || {
        echo "  WARN: llvm-cov failed for $target"
        return
    }

    echo "  OK    $out_dir/html/index.html"
}

# Parse arguments
FILTER_CRATE="${1:-}"
FILTER_TARGET="${2:-}"

echo "=== Fuzz Coverage Report Generator ==="
echo ""

for crate_name in "${!CRATE_DIRS[@]}"; do
    if [[ -n "$FILTER_CRATE" ]] && [[ "$crate_name" != "$FILTER_CRATE" ]]; then
        continue
    fi

    echo "[$crate_name]"
    targets="$(get_targets "${CRATE_DIRS[$crate_name]}" 2>/dev/null)" || {
        echo "  SKIP (cannot list targets)"
        continue
    }

    for target in $targets; do
        if [[ -n "$FILTER_TARGET" ]] && [[ "$target" != "$FILTER_TARGET" ]]; then
            continue
        fi
        run_coverage "$crate_name" "$target"
    done
    echo ""
done

echo "Done. Reports in: $COVERAGE_DIR/"
