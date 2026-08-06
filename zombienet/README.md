# Local Zombienet: Acurast parachain + Rococo relay

Spin up a local network with a **`rococo-local`** relay chain (2 validators) and the
**Acurast parachain** (id `2001`, chain `acurast-local`, 2 collators) using Zombienet's
`native` provider. Useful for manually testing runtime changes end-to-end (block
production, backing/inclusion, extrinsics).

- Relay: `charlie`, `dave` (BABE validators)
- Parachain `2001`: `Alice`, `Bob` (Acurast collators) — RPC/WS on `8082` / `8083`
- Token: `ACRST`, 12 decimals, ss58 prefix `42`
- Polkadot SDK tag: `polkadot-stable2606`

Everything is wired in [`config.toml`](./config.toml).

---

## Layout

```
zombienet/
├── config.toml            # the network definition (committed)
├── README.md              # this file
├── bin/
│   ├── acurast-node -> ../../target/release/acurast-node   # symlink (committed)
│   ├── acurast-node.bin    # optional prebuilt fallback (gitignored)
│   └── zombienet           # the zombienet CLI (gitignored — see setup)
├── relay/                  # from-source polkadot relay (gitignored — see setup)
│   └── bin/{polkadot,polkadot-execute-worker,polkadot-prepare-worker}
└── examples/
    └── deploy-job.mjs      # optional: transfers + a marketplace job deploy
```

Only `config.toml` and the `bin/acurast-node` symlink are committed. The `zombienet`
CLI and the whole `relay/` tree are gitignored, so **each developer builds/fetches
their own** per the setup below.

---

## Prerequisites

- **Rust toolchain** — pinned by the repo's [`rust-toolchain.toml`](../rust-toolchain.toml)
  (`1.97.0` + `wasm32v1-none` + `rust-src`); `rustup` installs it automatically.
- **Zombienet CLI** — download a release binary into `bin/zombienet` and `chmod +x` it:
  ```sh
  # pick the asset for your platform from
  # https://github.com/paritytech/zombienet/releases  (tested with v1.3.138)
  curl -L -o zombienet/bin/zombienet <release-asset-url>
  chmod +x zombienet/bin/zombienet
  ```
- **A `polkadot` relay binary** — see the next section. On **macOS/Apple Silicon there is
  no prebuilt** `polkadot`, so it must be built from source (below). On Linux x86_64 you
  may instead use `zombienet setup polkadot polkadot-parachain` or a Parity release.

---

## Setup

### 1. Build the Acurast node

From the repo root:

```sh
cargo build --release
```

Produces `target/release/acurast-node`, which `zombienet/bin/acurast-node` symlinks to.

### 2. Build the relay (`polkadot`) from source

The relay must match the SDK tag the node uses (`polkadot-stable2606`). This builds
`polkadot` plus its two PVF workers into `zombienet/relay/bin/`.

```sh
# Run from the repo root.
# WASM_BUILD_RUSTFLAGS: since stable2606 the runtime targets bare `wasm32v1-none` and
#   rust-lld no longer auto-imports undefined wasm symbols, so the relay runtimes fail
#   to link (`undefined symbol: ext_*`) without `--import-undefined`. The repo's
#   .cargo/config.toml sets this, but `cargo install` builds in a temp dir and does NOT
#   read local .cargo config, so it must be passed explicitly here.
# TMPDIR: `cargo install`'s scratch build tree is large (~15 GB) and it LEAVES it behind
#   on failure. Point it at a volume with plenty of space so a failed build can't fill /.
export TMPDIR="${TMPDIR:-/tmp}"                       # e.g. a big external volume
export WASM_BUILD_RUSTFLAGS="-Clink-arg=--import-undefined"

cargo install polkadot \
  --git https://github.com/paritytech/polkadot-sdk \
  --tag polkadot-stable2606 \
  --locked \
  --root "$(pwd)/zombienet/relay"

# Clean up the scratch tree afterwards:
rm -rf "$TMPDIR"/cargo-install*
```

Verify: `zombienet/relay/bin/polkadot --version` (expect `polkadot 1.24.0-…`).

---

## Run

Put both the relay binaries and the node/zombienet binaries on `PATH`, then spawn.
Run from the repo root:

```sh
export PATH="$(pwd)/zombienet/relay/bin:$(pwd)/zombienet/bin:$PATH"

# NOTE: do NOT pre-create the -d directory — zombienet prompts interactively
# ("Directory already exists; continue? (y/N)") and hangs on it. Use a fresh path,
# or omit -d to let zombienet use a temp dir.
zombienet -p native -d ./zombienet-run spawn zombienet/config.toml
```

Startup takes ~30–60 s. The parachain stays at block `#0` until the relay reaches
epoch 1, then begins authoring and getting included on the relay. When ready, Zombienet
prints **`Network launched 🚀`** and a table of endpoints.

| Node    | Role                 | Endpoint (fixed)              |
|---------|----------------------|-------------------------------|
| Alice   | parachain collator   | `ws://127.0.0.1:8082`         |
| Bob     | parachain collator   | `ws://127.0.0.1:8083`         |
| charlie | relay validator      | random port (see the table)   |
| dave    | relay validator      | random port (see the table)   |

Open in polkadot.js: <https://polkadot.js.org/apps/?rpc=ws://127.0.0.1:8082#/explorer>

### Check block production

```sh
curl -s -H 'Content-Type: application/json' \
  -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' \
  http://127.0.0.1:8082 | python3 -c "import sys,json;print('para best #', int(json.load(sys.stdin)['result']['number'],16))"
```

Per-node logs live under the `-d` directory (`./zombienet-run/{Alice,Bob,charlie,dave}.log`).

---

## Interact (optional)

[`examples/deploy-job.mjs`](./examples/deploy-job.mjs) demonstrates a full flow against a
running network: create a fresh account, transfer ACU to it from the genesis dev account
`//Alice`, then submit a `acurastMarketplace.deploy` job from the new account.

It needs `@polkadot/api`. Easiest is to run it from a checkout that already has it (e.g.
the `acurast-typescript-sdk`), or install locally:

```sh
cd zombienet/examples
npm init -y >/dev/null && npm install @polkadot/api @polkadot/util-crypto
node deploy-job.mjs
```

(The deployed job stays `Open` — matching to a processor requires a registered + attested
processor advertising capacity, which this minimal network does not include.)

---

## Teardown

`Ctrl-C` the `zombienet spawn` process (it stops all child nodes), then remove run data:

```sh
pkill -f "zombienet.*spawn"      # if it was backgrounded
rm -rf ./zombienet-run
```

---

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `rust-lld: undefined symbol: ext_*` while building the relay | Missing `WASM_BUILD_RUSTFLAGS="-Clink-arg=--import-undefined"` (see setup step 2). |
| `No space left on device` during relay build | `cargo install` scratch tree (~15 GB) filled the volume and is left behind on failure. Set `TMPDIR` to a big volume and `rm -rf "$TMPDIR"/cargo-install*`. |
| Zombienet hangs at `Directory already exists; continue? (y/N)` | The `-d` dir already existed. Use a fresh path or delete it first. |
| `ws-port flag was deprecated…` warning | Harmless. `ws_port` is deprecated; the config uses `rpc_port` (8082/8083). |
| `command not found: polkadot` / `acurast-node` | `PATH` doesn't include `zombienet/relay/bin` and `zombienet/bin` (see Run). |
| Parachain stuck at `#0` for >60 s | Give it a minute (waits for relay epoch 1). If it never advances, check `charlie.log`/`Alice.log` for backing errors. |
