#!/usr/bin/env bash
set -euo pipefail

# Instrumented fuzzing benchmark runner.
# Collects metrics for evaluation: coverage, exec/s, corpus size, crashes, time-to-crash.
#
# Usage:
#   ./fuzzing/bench.sh                              # all crates, 60s/target
#   ./fuzzing/bench.sh --crate parsers              # one crate
#   ./fuzzing/bench.sh --target fuzz_differential   # one target
#   FUZZ_TIMEOUT=300 ./fuzzing/bench.sh             # 5 min/target
#
# Output:
#   fuzzing/bench-results/<timestamp>/
#     metrics.jsonl        — one JSON object per target with all metrics
#     summary.txt          — human-readable summary table
#     <crate>/<target>/    — raw fuzzer output logs

FUZZ_TIMEOUT="${FUZZ_TIMEOUT:-60}"
FUZZ_TIMEOUT_PER_INPUT="${FUZZ_TIMEOUT_PER_INPUT:-10}"
FUZZ_RSS_LIMIT="${FUZZ_RSS_LIMIT:-2048}"
FUZZ_MAX_LEN="${FUZZ_MAX_LEN:-65536}"
FUZZ_CRATE="${FUZZ_CRATE:-}"
FUZZ_TARGET="${FUZZ_TARGET:-}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FUZZ_ENV="$SCRIPT_DIR/.fuzz-env"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
RESULTS_DIR="$SCRIPT_DIR/bench-results/$TIMESTAMP"

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

mkdir -p "$RESULTS_DIR"
METRICS_FILE="$RESULTS_DIR/metrics.jsonl"
> "$METRICS_FILE"

echo "=== Benchmark run: $TIMESTAMP"
echo "=== Timeout: ${FUZZ_TIMEOUT}s/target, RSS: ${FUZZ_RSS_LIMIT}MB, Max len: ${FUZZ_MAX_LEN}"
echo "=== Results: $RESULTS_DIR"
echo ""

# Extract metrics from libfuzzer output
extract_metrics() {
    local log_file="$1"

    # Final stats line: #NNNNN DONE cov: X ft: Y corp: Z/Kb exec/s: W
    local cov ft corpus_count corpus_size execs_per_sec total_execs
    cov=$(grep -oP 'cov: \K[0-9]+' "$log_file" | tail -1 || echo "0")
    ft=$(grep -oP 'ft: \K[0-9]+' "$log_file" | tail -1 || echo "0")
    corpus_count=$(grep -oP 'corp: \K[0-9]+' "$log_file" | tail -1 || echo "0")
    execs_per_sec=$(grep -oP 'exec/s: \K[0-9]+' "$log_file" | tail -1 || echo "0")
    total_execs=$(grep -oP '#\K[0-9]+' "$log_file" | tail -1 || echo "0")

    # NEW inputs discovered (coverage-increasing mutations)
    local new_inputs
    new_inputs=$(grep -c "NEW" "$log_file" 2>/dev/null || echo "0")

    # Peak RSS from libfuzzer
    local peak_rss
    peak_rss=$(grep -oP 'rss: \K[0-9]+Mb' "$log_file" | tail -1 || echo "0Mb")

    echo "$cov $ft $corpus_count $execs_per_sec $total_execs $new_inputs $peak_rss"
}

TOTAL_CRASHES=0
TOTAL_TARGETS=0

find "$REPO_ROOT/lib" -path "*/fuzz/Cargo.toml" -maxdepth 3 | sort | while read -r toml; do
    crate_dir="$(dirname "$(dirname "$toml")")"
    crate_name="$(basename "$crate_dir")"
    [ -n "$FUZZ_CRATE" ] && [ "$crate_name" != "$FUZZ_CRATE" ] && continue

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

        # Count pre-existing crashes
        artifact_dir="fuzz/artifacts/$target"
        pre_crashes=$(find "$artifact_dir" -type f 2>/dev/null | wc -l | tr -d ' ')

        # Count pre-existing corpus
        pre_corpus=$(find "$corpus_dir" -maxdepth 1 -type f 2>/dev/null | wc -l | tr -d ' ')

        # Seed corpus on first run
        if [ -d "$seeds_dir" ] && [ "$pre_corpus" -eq 0 ]; then
            cp "$seeds_dir"/* "$corpus_dir"/ 2>/dev/null || true
        fi

        DICT_ARG=()
        [ -n "$dict" ] && DICT_ARG=("-dict=$dict")

        # Run and capture output
        TARGET_RESULTS="$RESULTS_DIR/$crate_name/$target"
        mkdir -p "$TARGET_RESULTS"
        LOG_FILE="$TARGET_RESULTS/fuzz.log"

        echo "--- [$crate_name/$target] ${FUZZ_TIMEOUT}s..."
        START_TIME=$(date +%s%N)

        cargo +nightly fuzz run "$target" -- \
            -max_total_time="$FUZZ_TIMEOUT" \
            -timeout="$FUZZ_TIMEOUT_PER_INPUT" \
            -rss_limit_mb="$FUZZ_RSS_LIMIT" \
            -max_len="$FUZZ_MAX_LEN" \
            -print_final_stats=1 \
            "${DICT_ARG[@]}" > "$LOG_FILE" 2>&1 || true

        END_TIME=$(date +%s%N)
        ELAPSED_MS=$(( (END_TIME - START_TIME) / 1000000 ))

        # Count post-run crashes
        post_crashes=$(find "$artifact_dir" -type f 2>/dev/null | wc -l | tr -d ' ')
        new_crashes=$((post_crashes - pre_crashes))
        [ $new_crashes -lt 0 ] && new_crashes=0

        # Count post-run corpus
        post_corpus=$(find "$corpus_dir" -maxdepth 1 -type f 2>/dev/null | wc -l | tr -d ' ')

        # Extract libfuzzer metrics from log
        read -r cov ft corpus_count execs_per_sec total_execs new_inputs peak_rss <<< "$(extract_metrics "$LOG_FILE")"

        # Time to first crash (if any new crashes)
        time_to_crash="null"
        if [ "$new_crashes" -gt 0 ]; then
            first_crash_line=$(grep -n "SUMMARY.*ERROR\|Test unit written" "$LOG_FILE" | head -1 | cut -d: -f1)
            if [ -n "$first_crash_line" ]; then
                time_to_crash="$ELAPSED_MS"
            fi
        fi

        # Determine oracle type from source
        oracle="crash"
        if grep -q "assert_eq!.*oneshot.*stream\|assert_eq!.*streaming\|mismatch" "$target_file" 2>/dev/null; then
            oracle="differential"
        elif grep -q "round.trip\|encode_message.*decode_message" "$target_file" 2>/dev/null; then
            oracle="roundtrip"
        elif grep -q "shrank\|monoton\|>= .*_len" "$target_file" 2>/dev/null; then
            oracle="property"
        elif grep -q "silently dropped\|corrupted\|semantic\|BUG" "$target_file" 2>/dev/null; then
            oracle="semantic"
        elif grep -q "apply_event\|FuzzOp\|next_event" "$target_file" 2>/dev/null; then
            oracle="stateful"
        elif grep -q "score.*<=.*len\|remove_worker\|is_empty" "$target_file" 2>/dev/null; then
            oracle="consistency"
        fi

        # Write JSONL metrics
        cat >> "$METRICS_FILE" <<EOF
{"timestamp":"$TIMESTAMP","crate":"$crate_name","target":"$target","oracle":"$oracle","timeout_s":$FUZZ_TIMEOUT,"elapsed_ms":$ELAPSED_MS,"coverage_edges":$cov,"coverage_features":$ft,"corpus_size":$corpus_count,"corpus_pre":$pre_corpus,"corpus_post":$post_corpus,"execs_per_sec":$execs_per_sec,"total_execs":$total_execs,"new_inputs":$new_inputs,"peak_rss":"$peak_rss","crashes_pre":$pre_crashes,"crashes_post":$post_crashes,"crashes_new":$new_crashes,"time_to_crash_ms":$time_to_crash}
EOF

        if [ "$new_crashes" -gt 0 ]; then
            echo "    ** $new_crashes NEW CRASH(ES) **"
            TOTAL_CRASHES=$((TOTAL_CRASHES + new_crashes))
        fi
        echo "    cov=$cov ft=$ft corpus=$corpus_count exec/s=$execs_per_sec total=$total_execs"
    done
done

# Generate summary table
SUMMARY="$RESULTS_DIR/summary.txt"
{
    echo "Fuzzing Benchmark Results — $TIMESTAMP"
    echo "Timeout: ${FUZZ_TIMEOUT}s/target"
    echo ""
    printf "%-12s %-35s %-12s %6s %6s %8s %8s %8s %7s\n" \
        "Crate" "Target" "Oracle" "Cov" "Feat" "Corpus" "Exec/s" "Total" "Crashes"
    printf "%s\n" "$(printf '%.0s-' {1..115})"

    while IFS= read -r line; do
        crate=$(echo "$line" | grep -oP '"crate":"\K[^"]+')
        target=$(echo "$line" | grep -oP '"target":"\K[^"]+')
        oracle=$(echo "$line" | grep -oP '"oracle":"\K[^"]+')
        cov=$(echo "$line" | grep -oP '"coverage_edges":\K[0-9]+')
        ft=$(echo "$line" | grep -oP '"coverage_features":\K[0-9]+')
        corpus=$(echo "$line" | grep -oP '"corpus_size":\K[0-9]+')
        execs=$(echo "$line" | grep -oP '"execs_per_sec":\K[0-9]+')
        total=$(echo "$line" | grep -oP '"total_execs":\K[0-9]+')
        crashes=$(echo "$line" | grep -oP '"crashes_new":\K[0-9]+')

        printf "%-12s %-35s %-12s %6s %6s %8s %8s %8s %7s\n" \
            "$crate" "$target" "$oracle" "$cov" "$ft" "$corpus" "$execs" "$total" "$crashes"
    done < "$METRICS_FILE"

    echo ""
    echo "=== Totals ==="
    total_targets=$(wc -l < "$METRICS_FILE" | tr -d ' ')
    total_crashes=$(grep -oP '"crashes_new":\K[0-9]+' "$METRICS_FILE" | awk '{s+=$1}END{print s+0}')
    total_execs_all=$(grep -oP '"total_execs":\K[0-9]+' "$METRICS_FILE" | awk '{s+=$1}END{print s+0}')
    echo "Targets: $total_targets"
    echo "Total executions: $total_execs_all"
    echo "New crashes: $total_crashes"
} > "$SUMMARY"

cat "$SUMMARY"
echo ""
echo "=== Full metrics: $METRICS_FILE"
echo "=== Summary: $SUMMARY"
