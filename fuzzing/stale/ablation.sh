#!/usr/bin/env bash
set -euo pipefail

# Ablation study: compare oracle strategies head-to-head.
#
# Runs selected fuzz targets at multiple timeouts and records metrics,
# enabling direct comparison of:
#   1. Crash-only vs semantic oracles (same parser, different oracle)
#   2. Differential vs crash-only (reasoning parsers)
#   3. Stateful vs crash-only (kv-router data structures)
#   4. Time-to-bug scaling (does longer fuzzing find more?)
#
# Usage:
#   ./fuzzing/ablation.sh                  # default: 30s, 60s, 120s per target
#   ABLATION_TIMEOUTS="10 30 60 180 300" ./fuzzing/ablation.sh
#
# Output:
#   fuzzing/ablation-results/<timestamp>/
#     all_metrics.jsonl     — combined metrics across all runs
#     ablation_summary.txt  — formatted comparison table

ABLATION_TIMEOUTS="${ABLATION_TIMEOUTS:-30 60 120}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
RESULTS_DIR="$SCRIPT_DIR/ablation-results/$TIMESTAMP"
mkdir -p "$RESULTS_DIR"
ALL_METRICS="$RESULTS_DIR/all_metrics.jsonl"
> "$ALL_METRICS"

# Target groups for comparison
# Group 1: crash-only vs semantic (parsers)
CRASH_TARGETS="fuzz_parser_crash_oracle"
SEMANTIC_TARGETS="fuzz_parser_semantic"

# Group 2: differential vs crash-only (reasoning)
DIFFERENTIAL_TARGETS="fuzz_differential"
CRASH_REASONING="fuzz_parser_crash_oracle"

# Group 3: stateful vs crash-only (kv-router)
STATEFUL_TARGETS="fuzz_radix_tree_events fuzz_positional_indexer"
CONSISTENCY_TARGETS="fuzz_radix_tree_consistency"

# Group 4: property-based (parsers)
PROPERTY_TARGETS="fuzz_parser_properties"

echo "=== Ablation Study: $TIMESTAMP"
echo "=== Timeouts: $ABLATION_TIMEOUTS"
echo ""

for timeout in $ABLATION_TIMEOUTS; do
    echo "=========================================="
    echo "=== Timeout: ${timeout}s"
    echo "=========================================="

    # Clean artifacts before each timeout run to measure fresh crash discovery
    # (We only clean targets in our study, not all)
    for crate_dir in "$SCRIPT_DIR/../lib/parsers" "$SCRIPT_DIR/../lib/kv-router" "$SCRIPT_DIR/../lib/runtime"; do
        [ -d "$crate_dir/fuzz/artifacts" ] || continue
        for target_dir in "$crate_dir"/fuzz/artifacts/*/; do
            [ -d "$target_dir" ] || continue
            target="$(basename "$target_dir")"
            # Only clean targets in our study
            all_targets="$CRASH_TARGETS $SEMANTIC_TARGETS $DIFFERENTIAL_TARGETS $CRASH_REASONING $STATEFUL_TARGETS $CONSISTENCY_TARGETS $PROPERTY_TARGETS"
            echo "$all_targets" | tr ' ' '\n' | grep -qx "$target" && rm -f "$target_dir"/* 2>/dev/null || true
        done
    done

    # Run the benchmark with this timeout
    FUZZ_TIMEOUT="$timeout" "$SCRIPT_DIR/bench.sh" 2>&1 | tee "$RESULTS_DIR/bench_${timeout}s.log"

    # Find the most recent bench result and append to combined metrics
    latest_bench=$(ls -td "$SCRIPT_DIR/bench-results"/*/ 2>/dev/null | head -1)
    if [ -n "$latest_bench" ] && [ -f "$latest_bench/metrics.jsonl" ]; then
        cat "$latest_bench/metrics.jsonl" >> "$ALL_METRICS"
    fi
done

# Generate ablation summary
SUMMARY="$RESULTS_DIR/ablation_summary.txt"
{
    echo "Ablation Study Results — $TIMESTAMP"
    echo "Timeouts tested: $ABLATION_TIMEOUTS"
    echo ""
    echo "=== Oracle Strategy Comparison ==="
    echo ""
    printf "%-35s %-12s %6s %6s %8s %8s %7s\n" \
        "Target" "Oracle" "Time" "Cov" "Corpus" "Exec/s" "Crashes"
    printf "%s\n" "$(printf '%.0s-' {1..100})"

    while IFS= read -r line; do
        target=$(echo "$line" | grep -oP '"target":"\K[^"]+')
        oracle=$(echo "$line" | grep -oP '"oracle":"\K[^"]+')
        timeout=$(echo "$line" | grep -oP '"timeout_s":\K[0-9]+')
        cov=$(echo "$line" | grep -oP '"coverage_edges":\K[0-9]+')
        corpus=$(echo "$line" | grep -oP '"corpus_size":\K[0-9]+')
        execs=$(echo "$line" | grep -oP '"execs_per_sec":\K[0-9]+')
        crashes=$(echo "$line" | grep -oP '"crashes_new":\K[0-9]+')

        printf "%-35s %-12s %5ss %6s %8s %8s %7s\n" \
            "$target" "$oracle" "$timeout" "$cov" "$corpus" "$execs" "$crashes"
    done < "$ALL_METRICS"

    echo ""
    echo "=== Key Comparisons ==="
    echo ""

    # Compare crash-only vs semantic for parser targets
    echo "1. Crash-only vs Semantic oracles (parsers):"
    for t in $CRASH_TARGETS $SEMANTIC_TARGETS; do
        crashes=$(grep "\"target\":\"$t\"" "$ALL_METRICS" | grep -oP '"crashes_new":\K[0-9]+' | awk '{s+=$1}END{print s+0}')
        oracle=$(grep "\"target\":\"$t\"" "$ALL_METRICS" | head -1 | grep -oP '"oracle":"\K[^"]+' || echo "?")
        printf "   %-35s %-12s crashes=%s\n" "$t" "$oracle" "$crashes"
    done

    echo ""
    echo "2. Differential vs Crash-only (reasoning):"
    for t in $DIFFERENTIAL_TARGETS $CRASH_REASONING; do
        crashes=$(grep "\"target\":\"$t\"" "$ALL_METRICS" | grep -oP '"crashes_new":\K[0-9]+' | awk '{s+=$1}END{print s+0}')
        oracle=$(grep "\"target\":\"$t\"" "$ALL_METRICS" | head -1 | grep -oP '"oracle":"\K[^"]+' || echo "?")
        printf "   %-35s %-12s crashes=%s\n" "$t" "$oracle" "$crashes"
    done

    echo ""
    echo "3. Stateful vs Consistency (kv-router):"
    for t in $STATEFUL_TARGETS $CONSISTENCY_TARGETS; do
        crashes=$(grep "\"target\":\"$t\"" "$ALL_METRICS" | grep -oP '"crashes_new":\K[0-9]+' | awk '{s+=$1}END{print s+0}')
        oracle=$(grep "\"target\":\"$t\"" "$ALL_METRICS" | head -1 | grep -oP '"oracle":"\K[^"]+' || echo "?")
        printf "   %-35s %-12s crashes=%s\n" "$t" "$oracle" "$crashes"
    done

} > "$SUMMARY"

cat "$SUMMARY"
echo ""
echo "=== All metrics: $ALL_METRICS"
echo "=== Summary: $SUMMARY"
