#!/bin/bash

# Runs all benchmarks for all pallets, for a given runtime, provided by $1
# Should be run on a reference machine to gain accurate benchmarks
# current reference machine: https://github.com/paritytech/substrate/pull/5848

runtime="$1"

# Load all pallet names in an array.
PALLETS=($(
  ./target/release/acurast-node benchmark pallet --list --chain="${runtime}-dev" |\
    tail -n+2 |\
    cut -d',' -f1 |\
    sort |\
    uniq
))

echo "[+] Benchmarking ${#PALLETS[@]} pallets for runtime $runtime"

# Define the error file.
ERR_FILE="benchmarking_errors.txt"
# Delete the error file before each run.
rm -f $ERR_FILE
#touch "$ERR_FILE"
#chmod +rwx "$ERR_FILE"

mkdir ./scripts/local/weights

# Benchmark each pallet.
for PALLET in "${PALLETS[@]}"; do
  echo "[+] Benchmarking $PALLET for $runtime";

  if [ "$PALLET" == "pallet_acurast_marketplace" ]
  then
      # do first weight with hooks
      OUTPUT=$(
       ./target/release/acurast-node benchmark pallet \
        --chain="${runtime}-dev" \
        --steps=50 \
        --repeat=20 \
        --pallet="$PALLET" \
        --extrinsic="register, deregister, update_allowed_sources" \
        --execution=wasm \
        --wasm-execution=compiled \
        --output="./scripts/local/weights/pallet_acurast_marketplace_with_hooks.rs" \
        --template="../acurast-core/pallets/marketplace/src/weights_with_hooks.hbs" 2>&1
      )
      if [ $? -ne 0 ]; then
        echo "$OUTPUT" >> "$ERR_FILE"
        echo "[-] Failed to benchmark $PALLET. Error written to $ERR_FILE; continuing..."
      fi

      # do second weight without hooks
      OUTPUT=$(
        ./target/release/acurast-node benchmark pallet \
        --chain="${runtime}-dev" \
        --steps=50 \
        --repeat=20 \
        --pallet="$PALLET" \
        --extrinsic="advertise, delete_advertisement" \
        --execution=wasm \
        --wasm-execution=compiled \
        --output="./scripts/local/weights/pallet_acurast_marketplace_without_hooks.rs" \
        --template="../acurast-core/pallets/marketplace/src/weights_with_hooks.hbs" 2>&1
      )
      if [ $? -ne 0 ]; then
        echo "$OUTPUT" >> "$ERR_FILE"
        echo "[-] Failed to benchmark $PALLET. Error written to $ERR_FILE; continuing..."
      fi

  else
    output_file=""
      if [[ $PALLET == *"::"* ]]; then
        echo "$PALLET"
        # translates e.g. "pallet_foo::bar" to "pallet_foo_bar"
        output_file="${PALLET//::/_}.rs"
      fi

      OUTPUT=$(
        ./target/release/acurast-node benchmark pallet \
        --chain="${runtime}-dev" \
        --steps=50 \
        --repeat=20 \
        --pallet="$PALLET" \
        --extrinsic="*" \
        --execution=wasm \
        --wasm-execution=compiled \
        --output="./scripts/local/weights/${output_file}" 2>&1
      )
      if [ $? -ne 0 ]; then
        echo "$OUTPUT" >> "$ERR_FILE"
        echo "[-] Failed to benchmark $PALLET. Error written to $ERR_FILE; continuing..."
      fi
  fi
done

# Update the block and extrinsic overhead weights.
#echo "[+] Benchmarking block and extrinsic overheads..."
#OUTPUT=$(
#  ./target/production/acurast-node benchmark overhead \
#  --chain="${runtime}-dev" \
#  --execution=wasm \
#  --wasm-execution=compiled \
#  --weight-path="runtime/${runtime}/constants/src/weights/" \
#  --warmup=10 \
#  --repeat=100 \
#)
#if [ $? -ne 0 ]; then
#  echo "$OUTPUT" >> "$ERR_FILE"
#  echo "[-] Failed to benchmark the block and extrinsic overheads. Error written to $ERR_FILE; continuing..."
#fi

# Check if the error file exists.
if [ -f "$ERR_FILE" ]; then
  echo -e "[-] Some benchmarks failed, printing log...\n"
  cat $ERR_FILE
else
  echo "[+] All benchmarks passed."
fi
