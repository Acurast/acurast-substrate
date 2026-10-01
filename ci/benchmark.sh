#!/usr/bin/env bash
#
# Runs INSIDE the benchmarking container (see ci/docker-compose.benchmark.yml),
# bind-mounted at /ci/benchmark.sh. The `benchmarking` stage of .gitlab-ci.yml
# ships this repo to the bare-metal box over SSH and starts the container there.
#
# Kept as a file (rather than inlined into `ssh ... "<command>"`) so that the
# `*` pallet/extrinsic selectors are quoted exactly once. Inlining them meant the
# outer quotes were consumed by the SSH command string and the remote shell
# glob-expanded `*` against its working directory.
#
# Usage: benchmark.sh <chain-id> [steps] [repeat]

set -euo pipefail

CHAIN="${1:?usage: benchmark.sh <chain-id> [steps] [repeat]}"
STEPS="${2:-50}"
REPEAT="${3:-20}"
# ref_time scale factor, applied to pallet weights and to `benchmark overhead` (see below).
SCALE_NUM="${SCALE_NUM:-3}"
SCALE_DEN="${SCALE_DEN:-2}"

NODE="${NODE:-/usr/local/bin/acurast-node}"
OUT=/bench/benchmarks

# What the caller asked docker for, forwarded by the compose file. The checks
# below assert that the cgroup actually got it, which catches a cpuset that was
# silently truncated, a limit that never applied, and SMT siblings counted as
# cores.
#
# Note which direction is dangerous: weights generated on hardware SLOWER than
# the Polkadot validator reference (fewer or weaker cores) come out too high,
# which is conservative and safe. Only faster hardware understates execution cost
# and lets blocks overrun. So a box with fewer cores than the reference 8 is fine
# to benchmark on; `benchmark machine` output in machine.txt records how far off
# the reference this particular box is.
CPUSET_REQUESTED="${BENCH_CPUSET_REQUESTED:-}"
MEM_REQUESTED="${BENCH_MEM_REQUESTED:-}"

# Bytes from a docker-style size ("32g", "512m", "1024k", or plain bytes).
parse_size() {
	local v="$1" num unit
	num="$(printf '%s' "$v" | tr -dc '0-9')"
	unit="$(printf '%s' "$v" | tr -dc 'bBkKmMgG' | tr '[:upper:]' '[:lower:]' | cut -c1)"
	case "$unit" in
	g) echo $((num * 1024 * 1024 * 1024)) ;;
	m) echo $((num * 1024 * 1024)) ;;
	k) echo $((num * 1024)) ;;
	*) echo "$num" ;;
	esac
}

# Overridable only so the SMT check can be exercised against a fake topology;
# in a real run this is the host's sysfs.
SYSFS_CPU="${SYSFS_CPU:-/sys/devices/system/cpu}"

mkdir -p "$OUT" /bench/db

# Expands a cpuset list ("0-3,8,12-13") into one id per line.
expand_cpuset() {
	local spec="$1" part lo hi
	local IFS=,
	for part in $spec; do
		case "$part" in
		*-*)
			lo="${part%-*}"
			hi="${part#*-}"
			seq "$lo" "$hi"
			;;
		*) echo "$part" ;;
		esac
	done
}

# Fails unless the cgroup's cpuset is exactly the one requested and consists of
# whole physical cores.
#
# /sys is the host's, so thread_siblings_list reflects real topology even though
# the cgroup exposes only a subset of CPUs.
#
# The requirement is sibling-COMPLETENESS, not sibling-absence: on an SMT host,
# owning one thread of a core while the host schedules work on the other thread
# means the two compete for one core's execution units, and the benchmark absorbs
# whatever the host happened to be doing. Owning both threads of each core makes
# the pinned cores genuinely private. A half-owned core is therefore an error,
# even though it superficially looks like the more conservative choice.
check_topology() {
	local cpuset cpus count cpu siblings sib want want_count missing cores
	cpuset="$(cat /sys/fs/cgroup/cpuset.cpus.effective 2>/dev/null ||
		cat /sys/fs/cgroup/cpuset/cpuset.effective_cpus 2>/dev/null || echo "")"

	if [ -z "$cpuset" ]; then
		echo "FATAL: cannot read the container's cpuset from /sys/fs/cgroup" >&2
		return 1
	fi

	cpus="$(expand_cpuset "$cpuset")"
	count="$(echo "$cpus" | wc -l | tr -d ' ')"
	echo "pinned cpuset: $cpuset ($count CPUs)"

	if [ -z "$CPUSET_REQUESTED" ]; then
		echo "FATAL: BENCH_CPUSET_REQUESTED not passed in; cannot tell whether the" >&2
		echo "       cgroup got the cpuset that was asked for" >&2
		return 1
	fi
	want="$(expand_cpuset "$CPUSET_REQUESTED")"
	want_count="$(echo "$want" | wc -l | tr -d ' ')"
	if [ "$count" -ne "$want_count" ]; then
		echo "FATAL: requested $want_count CPUs ($CPUSET_REQUESTED) but the cgroup has" >&2
		echo "       $count ($cpuset). The box may have fewer CPUs than BENCH_CPUSET names." >&2
		return 1
	fi

	# Always record the real mapping: it is the only way to choose BENCH_CPUSET
	# for a given box without guessing how Linux enumerated the threads.
	echo "sibling groups of the pinned CPUs:"
	for cpu in $cpus; do
		siblings="$(tr ',' ' ' <"$SYSFS_CPU/cpu$cpu/topology/thread_siblings_list" 2>/dev/null || echo "$cpu")"
		echo "  cpu$cpu -> [$siblings]"
	done

	missing=""
	for cpu in $cpus; do
		siblings="$(tr ',' ' ' <"$SYSFS_CPU/cpu$cpu/topology/thread_siblings_list" 2>/dev/null || echo "$cpu")"
		for sib in $siblings; do
			if ! echo "$cpus" | grep -qx "$sib"; then
				missing="$missing cpu$sib(sibling of cpu$cpu)"
			fi
		done
	done
	if [ -n "$missing" ]; then
		echo "FATAL: the cpuset owns only part of a physical core. Missing:$missing" >&2
		echo "       The host can schedule work on the missing thread(s), which then" >&2
		echo "       competes for the same core and distorts the measurement." >&2
		echo "       Add the listed CPUs to BENCH_CPUSET (see the sibling groups above)," >&2
		echo "       or disable SMT on the box." >&2
		return 1
	fi

	# One group per physical core, so unique sibling lists == core count.
	cores="$(for cpu in $cpus; do
		cat "$SYSFS_CPU/cpu$cpu/topology/thread_siblings_list" 2>/dev/null || echo "$cpu"
	done | sort -u | wc -l | tr -d ' ')"
	echo "topology OK: $count threads = $cores whole physical cores"
}

# Fails when the memory limit is missing (the container could use the whole box)
# or differs from what was requested.
check_memory() {
	local limit want
	limit="$(cat /sys/fs/cgroup/memory.max 2>/dev/null ||
		cat /sys/fs/cgroup/memory/memory.limit_in_bytes 2>/dev/null || echo "")"

	if [ -z "$limit" ] || [ "$limit" = "max" ]; then
		echo "FATAL: no memory limit on the container — set BENCH_MEM" >&2
		return 1
	fi
	echo "memory limit: $limit bytes"

	if [ -z "$MEM_REQUESTED" ]; then
		echo "FATAL: BENCH_MEM_REQUESTED not passed in; cannot verify the limit" >&2
		return 1
	fi
	want="$(parse_size "$MEM_REQUESTED")"
	if [ "$limit" -ne "$want" ]; then
		echo "FATAL: requested $MEM_REQUESTED ($want bytes), cgroup has $limit" >&2
		return 1
	fi
	echo "memory OK"
}

echo "=== Container resource conformance ==="
{
	check_topology
	check_memory
	echo
	echo "nproc visible: $(nproc)"
	echo "host cpu model: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ *//' || true)"
	echo "host memory:"
	head -3 /proc/meminfo
} 2>&1 | tee "$OUT/hardware.txt"

# Records whether this machine actually clears the Polkadot reference-hardware
# thresholds (BLAKE2-256, SR25519-Verify, memory copy, disk seq/rnd write).
#
# `--allow-fail` keeps a marginal result from killing a 2h pipeline, but the
# output is archived so a failing run is visible. Drop `--allow-fail` once the
# box is stable, so weights can never be generated on a machine that does not
# meet the reference spec.
echo "=== benchmark machine ==="
"$NODE" benchmark machine \
	--chain="$CHAIN" \
	--base-path=/bench/db \
	--allow-fail \
	2>&1 | tee "$OUT/machine.txt"

# --heap-pages=2048 is the executor's own default (DEFAULT_HEAP_ALLOC_PAGES in
# substrate/client/executor/common/src/wasm_runtime.rs), so the benchmark runs under the same
# memory conditions as production. It is pinned rather than omitted so that a future change to
# that default cannot silently shift every weight.
#
# This said 4096 until 2026-08-05, on the incorrect claim that 4096 was the production value.
# That made the run differ from production and from the previous weight set, muddying the
# comparison between them.
#
# --execution=wasm is deliberately absent: it no longer selects anything. The stable2606 CLI
# accepts it and ignores it, warning "Argument `--execution` is deprecated. Its value of `wasm`
# has on effect." Passing it only adds noise to the log and to the generated file headers.

# These runtime log targets emit one line per benchmark repetition, which on a --steps=50
# --repeat=20 run adds up to ~2MB and blew GitLab's 4MB job-log cap on 2026-09-02 ("Job's log
# exceeded limit of 4194304 bytes"). The cap silently truncates everything after it, so the failure
# summary, the high-repeat pass, the overhead step and the scaling step were all lost from the log
# while the job kept running. None of them says anything about the weights:
#
#   * runtime::storage_reclaim_pallet -- "Node-side PoV size higher than runtime proof size weight".
#     The benchmark harness does not do real block accounting, so runtime BlockWeight is 0 while the
#     node-side recorder reports real bytes. 6885 lines, 1.0MB.
#   * runtime::xcmp-queue-migration -- "Message dropped: too big", from the v3->v4 migration path the
#     xcmp-queue benchmarks drive deliberately. Logged at error level, so it needs `off`, not
#     `error`. 30892 lines, 0.9MB.
#   * pallet_collator_selection -- "assembling new collators for new session N", one per session
#     rotation the benchmarks trigger. 2202 lines, 0.1MB.
#   * acurast_<network>_runtime::apis -- "WARNING: benchmark error overridden - <name>", one per
#     iteration of the --min-duration loop. A `BenchmarkError::Override` benchmark returns
#     instantly, so the CLI spins for the whole minimum duration re-running it: 37136 lines,
#     2.2MB, for `teleport_assets` alone on 2026-09-02, and there are four overridden
#     extrinsics. The information is not lost -- an overridden benchmark is unmistakable in
#     the generated file, where its ref_time is Weight::MAX (18_446_744_073_709_551_000).
#     Only the compiled network's target does anything; the other two are inert.
#   * xcm::benchmarking -- "try_origin failed", logged by the pallet_xcm benchmarks right before
#     they return the same `BenchmarkError::Override`, so it repeats with the line above.
#
# Everything else keeps its default level, so the CLI's own progress and summary output is untouched.
BENCH_LOG_FILTER="${BENCH_LOG_FILTER:-runtime::storage_reclaim_pallet=off,runtime::xcmp-queue-migration=off,pallet_collator_selection=off,xcm::benchmarking=off,acurast_mainnet_runtime::apis=off,acurast_kusama_runtime::apis=off,acurast_rococo_runtime::apis=off}"

# Collapses runs of identical consecutive log lines, ignoring their timestamp.
#
# The log filter above names the targets known to flood, but three separate ones turned up on
# 2026-09-02 alone, each discovered only by blowing GitLab's 4MB cap and truncating the run's tail --
# including the failure summary, which is the one thing needed to diagnose it. This makes the next
# unknown one cost a single line instead of the rest of the log.
collapse_repeats() {
	awk '{
		msg = $0
		sub(/^[0-9][0-9-]* [0-9:]+ /, "", msg)
		if (NR > 1 && msg == prev) { n++; next }
		if (n) { printf "  [previous line repeated %d more times]\n", n; n = 0 }
		prev = msg
		print
		fflush()
	}
	END { if (n) printf "  [previous line repeated %d more times]\n", n }'
}

echo "=== benchmark pallet ==="
"$NODE" benchmark pallet \
	--chain="$CHAIN" \
	--wasm-execution=compiled \
	--heap-pages=2048 \
	--pallet '*' \
	--extrinsic '*' \
	--steps="$STEPS" \
	--repeat="$REPEAT" \
	--log="$BENCH_LOG_FILTER" \
	--output="$OUT/" \
	2>&1 | collapse_repeats | tee "$OUT/pallet.log"

# `benchmark overhead` measures BlockExecutionWeight and ExtrinsicBaseWeight -- the per-block and
# per-extrinsic cost that RuntimeBlockWeights adds on top of every dispatch. Without it those two
# stay at the stock upstream placeholders (5ms and 125us), which is what the SDK parachains ship,
# but it does not mix with the ref_time scaling below: per-call weights would be chain-measured and
# scaled while the base weights stayed an unmeasured guess at the unscaled scale.
#
# Two wrinkles, both of which the CLI cannot work around on its own:
#
#   * --para-id is required. Without it the block builder omits the parachain inherent and
#     cumulus-pallet-parachain-system panics with "included head not present in relay storage proof".
#     It is read back out of the chain spec so this stays correct for every network.
#
#   * The extrinsic half signs a `System::remark` as Alice, who holds no balance in any of our chain
#     specs, so it fails with Invalid(Payment). --genesis-patch cannot help: it is refused alongside
#     --chain, and --runtime on its own trips the node's load_spec("") lookup. So we export the spec,
#     endow Alice in it, and benchmark against that copy. Nothing else in genesis is touched.
#
# --mul applies the same hardware correction the perl pass below applies to pallet weights, recorded
# in the generated header as WEIGHT-MUL. Those two files use
# `WEIGHT_REF_TIME_PER_NANOS.saturating_mul(N)` rather than a bare `Weight::from_parts(N, ...)`, so
# the perl pass does not match them and cannot double-apply it.
#
# Set BENCH_OVERHEAD to the empty string to skip.
BENCH_OVERHEAD="${BENCH_OVERHEAD:-1}"

if [ -n "$BENCH_OVERHEAD" ]; then
	echo "=== benchmark overhead ==="
	# --base-path is required: the container runs as a bare uid with no writable home, so the
	# default base path is unopenable and build-spec fails with
	# `Io(Os { code: 13, kind: PermissionDenied })` (2026-09-02). `benchmark pallet` does not
	# need one; `build-spec` and `benchmark overhead` both do.
	"$NODE" build-spec --chain="$CHAIN" --base-path=/bench/db > "$OUT/spec.json"

	# Endow //Alice by textual insertion rather than a JSON decode/encode round-trip. The genesis
	# holds u128 balances larger than 2^64 (e.g. acurastHyperdriveToken.initialEthTokenAllocation),
	# and perl -- JSON::PP with allow_bignum included -- reads those into a double and writes them
	# back as `7.623964e+19`, which the runtime rejects with "Invalid JSON blob". Inserting one array
	# element leaves every other byte of the spec untouched.
	perl -0777 -i -pe '
		s{("balances"\s*:\s*\{\s*"balances"\s*:\s*\[)}
		 {$1\n            [\n              "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",\n              1000000000000000000000\n            ],}
	' "$OUT/spec.json"
	grep -q 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY "$OUT/spec.json" \
		|| { echo "failed to endow //Alice in $OUT/spec.json" >&2; exit 1; }

	PARA_ID="$(perl -0777 -ne 'print $1 if /"para_id"\s*:\s*(\d+)/' "$OUT/spec.json")"
	[ -n "$PARA_ID" ] || { echo "no para_id in $OUT/spec.json" >&2; exit 1; }
	echo "  para-id $PARA_ID, weight-mul $SCALE_NUM/$SCALE_DEN"

	"$NODE" benchmark overhead \
		--chain="$OUT/spec.json" \
		--base-path=/bench/db \
		--para-id="$PARA_ID" \
		--wasm-execution=compiled \
		--mul="$(perl -e 'printf "%.4f", $ARGV[0] / $ARGV[1]' "$SCALE_NUM" "$SCALE_DEN")" \
		--weight-path="$OUT" \
		2>&1 | collapse_repeats | tee "$OUT/overhead.log"
fi

# A second pass at a higher --repeat for pallets whose weights come out of a least-squares fit over a
# very short component range, where 20 repeats leave the intercept/slope split dominated by noise.
#
# `pallet_acurast_marketplace` is what forced this. `propose_matching` ranges over
# `x in 1 .. MaxProposedMatches`, and MaxProposedMatches is 3, so the fit gets three points no matter
# what --steps says. The 2026-08-27 run split it as `5_421_264_000 + 2_377_732_275*x`; the previous one
# as `10_723_827 + 5_824_646_942*x`. Nearly the same total at x=2, but the new intercept is worth two
# thirds of a whole match, and the standard error on the slope went from 0.13% of it to 2.2%. The
# intercept is the part that matters: `propose_matching` refunds skipped matches down to
# `WeightInfo::propose_matching(processed)` (pallets/marketplace/src/lib.rs), so a fat intercept is
# charged in full even when `processed` is 0 — the contended case the refund exists for.
#
# Repeats only buy 1/sqrt(n), so they narrow the intercept but do not pin it down: the 2026-09-29
# --repeat=100 run fit `3964 + 1767*x` µs to means of 4027/8174/12690 µs. This pass therefore uses
# --output-analysis=median-slopes, which stayed within 3% of those means in both runs.
#
# Whole pallets are rerun (`--extrinsic '*'`) because the CLI writes one file per pallet, so a
# single-extrinsic rerun would truncate the rest of that file. Cheap extrinsics in the same pallet pay
# the higher repeat too; they are microseconds, so it is the two matching benchmarks that cost.
#
# Runs before the scaling step, so files this overwrites are still scaled exactly once.
#
# Set PALLETS_HIGH_REPEAT to the empty string to skip the pass.
REPEAT_HIGH="${REPEAT_HIGH:-100}"
PALLETS_HIGH_REPEAT="${PALLETS_HIGH_REPEAT-pallet_acurast_marketplace}"

if [ -n "$PALLETS_HIGH_REPEAT" ]; then
	for pallet in $PALLETS_HIGH_REPEAT; do
		echo "=== benchmark pallet $pallet (--repeat=$REPEAT_HIGH, median slopes) ==="
		"$NODE" benchmark pallet \
			--chain="$CHAIN" \
			--wasm-execution=compiled \
			--heap-pages=2048 \
			--pallet "$pallet" \
			--extrinsic '*' \
			--steps="$STEPS" \
			--repeat="$REPEAT_HIGH" \
			--output-analysis=median-slopes \
			--log="$BENCH_LOG_FILTER" \
			--output="$OUT/" \
			2>&1 | collapse_repeats | tee "$OUT/pallet-$pallet.log"
	done
fi

# Scale ref_time up, because this box is faster than the hardware the weights must protect.
#
# `benchmark machine` (machine.txt) puts it at 150% of the reference BLAKE2-256 minimum, 171% of
# SR25519-Verify and 150% of Memory Copy. Taken at face value, weights measured here understate
# execution cost for a validator sitting at the reference floor, and a block built to fit them could
# overrun on that validator. SCALE_NUM/SCALE_DEN compensates; 3/2 keys on the two metrics that agree
# at 1.50x, and lands close to the previous weight set, which was generated on a box that happened to
# be ~1.5x slower than this one.
#
# Only ref_time is scaled:
#   * proof_size is a byte count, independent of how fast the machine is, so the second argument of
#     `from_parts` and its per-component slopes are left alone. Inflating PoV would waste block
#     space and misstate what the extrinsic actually writes.
#   * `T::DbWeight` contributions are left alone: RocksDbWeight is itself a constant derived from
#     reference hardware, so scaling it here would apply the correction twice.
#
# The raw measurements stay visible in the "Minimum execution time" comments and in pallet.log, so
# the transformation can always be checked against the unscaled source.
if [ "$SCALE_NUM" != "$SCALE_DEN" ]; then
	echo "=== scaling ref_time by $SCALE_NUM/$SCALE_DEN ==="
	for f in "$OUT"/*.rs; do
		[ -e "$f" ] || continue
		# Scaling twice would silently square the factor, so the note this script inserts also acts
		# as the marker that a file has already been processed.
		if grep -q "were scaled by" "$f"; then
			echo "  already scaled, skipping $(basename "$f")"
			continue
		fi
		SCALE_NUM="$SCALE_NUM" SCALE_DEN="$SCALE_DEN" perl -i -pe '
			BEGIN {
				$num = $ENV{SCALE_NUM};
				$den = $ENV{SCALE_DEN};
				# u64::MAX divided by the multiplier: at or above this, the intermediate $plain * $num
				# overflows the 64-bit integer arithmetic perl uses and silently becomes a double, which group() then
				# renders as scientific notation -- not valid Rust.
				$overflow_limit = 18446744073709551615 / $num;
				# Groups digits with "_" the way the generated files already write large numbers.
				sub group {
					my $n = reverse shift;
					$n =~ s/(\d{3})(?=\d)/$1_/g;
					return scalar reverse $n;
				}
			}
			# First argument of from_parts is ref_time, second is proof_size. A zero ref_time marks a
			# proof-size-only term, which must stay untouched.
			s{Weight::from_parts\((\d[\d_]*),\s*(\d[\d_]*)\)}{
				my ($ref, $proof) = ($1, $2);
				my $plain = $ref;
				$plain =~ s/_//g;
				if ($plain eq "0") {
					"Weight::from_parts($ref, $proof)";
				} elsif ($plain + 0 >= $overflow_limit) {
					# `Weight::MAX`, written by the CLI when a benchmark returns
					# `BenchmarkError::Override` because the runtime config makes the extrinsic
					# impossible to set up (e.g. `XcmExecuteFilter = Nothing`). It is a sentinel
					# meaning "unusable", not a measurement, so there is nothing to scale.
					warn "  WARNING: $ref left un-scaled (Weight::MAX override) in $ARGV\n";
					"Weight::from_parts($ref, $proof)";
				} else {
					# Round up: never scale a weight down through integer truncation.
					my $scaled = int(($plain * $num + $den - 1) / $den);
					"Weight::from_parts(" . group($scaled) . ", $proof)";
				}
			}gex;
			# Record the transformation in the file itself, so a reader of the committed weights sees
			# that these numbers are not verbatim CLI output.
			if (/^#!\[cfg_attr\(rustfmt, rustfmt_skip\)\]/ && !$done) {
				$done = 1;
				$_ = "// NOTE: `ref_time` values below were scaled by $num/$den after generation by\n"
				   . "// ci/benchmark.sh, to compensate for benchmarking hardware measuring ~1.5x the\n"
				   . "// Polkadot validator reference minimums. `proof_size` and DbWeight are unscaled.\n"
				   . "// The unscaled measurements remain in the \"Minimum execution time\" comments.\n"
				   . $_;
			}
		' "$f"
		echo "  scaled $(basename "$f")"
	done
fi

echo "=== Done. Artifacts in $OUT ==="
ls -la "$OUT"
