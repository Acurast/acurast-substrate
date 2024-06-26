## Contracts

```
State: 5Fw2pUR67Vic9ah7dCffMLx4qxQcZG8uYE32Ci31SUvXrefP
Validator: 5G6encHfCXmAhfAYcesSoBhGWraWzQ4wFXUgzFA7LxSSmKtu
Proxy: 5FN48UFaPYj2kYXGz9yLYka2FErzGCaEYGikxxCbxrAvEmvp
Consumer:
```

## Contract CLI

[Docs](https://use.ink/getting-started/deploy-your-contract/)

Dry run with
```sh
cargo +stable contract upload --manifest-path ibc-ink-4/Cargo.toml --url wss://ws.test.azero.dev/ --suri "<MNEMONIC>"
```

Execute with
```sh
cargo +stable contract upload --manifest-path ibc-ink-4/Cargo.toml --url wss://ws.test.azero.dev/ --suri "<MNEMONIC>" -x > contract
cargo +stable contract instantiate --manifest-path ibc-ink-4/Cargo.toml --url wss://ws.test.azero.dev/ --constructor default --suri "<MNEMONIC>" -x --skip-confirm > instantiaten
```

See [contracts.md](contracts.md).

## Deployed Contracts

### Local testing:

Contract UI: https://ui.use.ink/?rpc=ws://127.0.0.1:9944

### AlephZero Testnet

Contract UI: https://ui.use.ink/?rpc=wss://ws.test.azero.dev

https://ui.use.ink/contract/5Cf4eGfxzBhqE5HvvzQe2muueuGukffqdMEqzUEBJWd4mpJR

(Configure wss://ws.test.azero.dev/ as endpoint or choose from chain dropdown)

[Last Output from uploading contract](./contract)

[Output from intantiation](./instantiation)

## Development

Check out all [shortcut functions](https://docs.rs/ink_env/5.0.0/ink_env/#functions) exposed by the [`Environment`](https://use.ink/basics/environment-functions/).
