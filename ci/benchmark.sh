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

# --heap-pages=4096 matches the heap allocation the runtime is executed with in
# production; without it the benchmark runs under different memory conditions
# than the weights are meant to describe.
#
# --execution=wasm is deliberately absent: native execution was removed from
# the SDK, so the flag is rejected by the polkadot-v1.18.5 CLI.
echo "=== benchmark pallet ==="
"$NODE" benchmark pallet \
	--chain="$CHAIN" \
	--wasm-execution=compiled \
	--heap-pages=4096 \
	--pallet '*' \
	--extrinsic '*' \
	--steps="$STEPS" \
	--repeat="$REPEAT" \
	--output="$OUT/" \
	2>&1 | tee "$OUT/pallet.log"

echo "=== Done. Artifacts in $OUT ==="
ls -la "$OUT"
