// Demo: against a running local zombienet (see ../README.md), create a fresh account,
// transfer ACU to it from the genesis dev account //Alice, then deploy a marketplace job
// from the new account.
//
//   cd zombienet/examples
//   npm init -y && npm install @polkadot/api @polkadot/util-crypto
//   node deploy-job.mjs
//
// Parachain (Acurast) RPC/WS defaults to ws://127.0.0.1:8082.

import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import { cryptoWaitReady, mnemonicGenerate } from '@polkadot/util-crypto';

const WS = process.env.ACURAST_WS || 'ws://127.0.0.1:8082';
const DECIMALS = 12n;
const ACU = (n) => BigInt(n) * 10n ** DECIMALS;
const fmt = (raw) =>
  (Number(BigInt(raw)) / 1e12).toLocaleString(undefined, { maximumFractionDigits: 4 }) + ' ACU';

// Submit a signed extrinsic and resolve once in a block, surfacing dispatch errors.
function send(api, tx, signer, label) {
  return new Promise((resolve, reject) => {
    tx.signAndSend(signer, ({ status, dispatchError, events }) => {
      if (!status.isInBlock) return;
      if (dispatchError) {
        const msg = dispatchError.isModule
          ? (() => {
              const d = api.registry.findMetaError(dispatchError.asModule);
              return `${d.section}.${d.name}: ${d.docs.join(' ')}`;
            })()
          : dispatchError.toString();
        reject(new Error(`${label} FAILED — ${msg}`));
      } else {
        resolve({ block: status.asInBlock.toHex(), events });
      }
    }).catch(reject);
  });
}

async function main() {
  await cryptoWaitReady();
  const api = await ApiPromise.create({ provider: new WsProvider(WS), noInitWarn: true });
  console.log(`Connected to ${await api.rpc.system.chain()}`);

  const keyring = new Keyring({ type: 'sr25519', ss58Format: 42 });
  const alice = keyring.addFromUri('//Alice');

  // ---- new account ----
  const mnemonic = mnemonicGenerate();
  const newAcct = keyring.addFromUri(mnemonic);
  console.log(`\nNew account: ${newAcct.address}`);
  console.log(`  (mnemonic: ${mnemonic})`);

  const bal = async (addr) => (await api.query.system.account(addr)).data.free.toString();
  console.log(`\nNew balance: ${fmt(await bal(newAcct.address))} (before)`);

  // ---- test transfers (>= 100 ACU total) ----
  console.log('\n=== Test transfers //Alice -> new account ===');
  for (const amt of [ACU(100), ACU(400)]) {
    const r = await send(api, api.tx.balances.transferKeepAlive(newAcct.address, amt), alice, `transfer ${fmt(amt)}`);
    console.log(`  transferred ${fmt(amt)}  (inBlock ${r.block.slice(0, 12)}…)`);
  }
  console.log(`New balance: ${fmt(await bal(newAcct.address))} (after transfers)`);

  // ---- build a valid job registration ----
  // Constraints enforced by the marketplace register hook:
  //   duration > 0, duration < interval, start_time >= chain-now (ms),
  //   0 < execution_count <= MAX, 0 < slots <= MaxSlots.
  //   execution_count = ((end-start-1)/interval)+1  -> here == 1.
  const now = Date.now();
  const startTime = now + 120_000; // 2 min in the future
  const schedule = {
    duration: 30_000,
    startTime,
    endTime: startTime + 60_000,
    interval: 120_000, // > (end-start) => 1 execution; and > duration
    maxStartDelay: 0,
  };
  const scriptHex =
    '0x' + Buffer.from('ipfs://QmQgcdqZPsnnnuvBVQeQgTy3ccQknGbnRpHWZMQnbyYMvo').toString('hex');
  const reward = ACU(10); // per slot & execution; total locked = reward * slots * execution_count

  const registration = {
    script: scriptHex,
    allowedSources: null,
    allowOnlyVerifiedSources: false,
    schedule,
    memory: 1_000_000,
    networkRequests: 0,
    storage: 0,
    requiredModules: [],
    extra: {
      requirements: {
        assignmentStrategy: { Single: null }, // Single(None) => register only, no instant match
        slots: 1,
        reward: reward.toString(),
        minReputation: null,
        processorVersion: null,
        runtime: 'NodeJS',
      },
    },
  };

  console.log('\n=== acurastMarketplace.deploy from new account ===');
  const deployTx = api.tx.acurastMarketplace.deploy(
    registration,
    { Immutable: null }, // mutability
    null, // reuse_keys_from
    null, // min_metrics
  );
  const res = await send(api, deployTx, newAcct, 'deploy');
  console.log(`✅ deploy included in block ${res.block}`);
  res.events.forEach(({ event }) => {
    if (event.section.startsWith('acurast') || (event.section === 'balances' && event.method === 'Transfer')) {
      console.log(`   ${event.section}.${event.method}  ${JSON.stringify(event.data.toHuman())}`);
    }
  });
  console.log(`\nNew balance: ${fmt(await bal(newAcct.address))} (after deploy — reward locked + fees)`);

  await api.disconnect();
  process.exit(0);
}

main().catch((e) => {
  console.error('ERROR:', e.message || e);
  process.exit(1);
});
