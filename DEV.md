# Deveopment Notes

## Configuration

The code's execution may rely on external data, e.g., Ethereum JSON-RPC or Reth database.
There is a configuration where users can specify such dependencies.

The configuration declared in `src/config/flags.rs`;
An example is below:

```toml
[reth]
datadir = "/path/to/datadir"

[jsonrpc]
endpoint = "http://localhost:8545"
cloudflare_client_id = "xxx"
cloudflare_client_secret = "xxx"
```

All fields and sections are optional.

## Types

For low level types, e.g., Bytes, U256, etc, we use those provided by `revm` crate.

For high level types, e.g., Address, TxHash, Log, Receipt, etc, we use those provided in `reth` crate. (Some types in `reth` are aliases of those in `revm`)

Normally, we don't use types provided in `ethers` crate.
Should they are needed, conversions functions are provided in `./src/utils/conversion.rs`.

## Testing

There are three groups of tests, grouped by rust modules:
- `tests_nodep`: tests that do not rely on any external dependencies.
- `tests_with_db`: tests that rely on Reth database, i.e., the database must present for the tests to be executed.
- `tests_with_jsonrpc`: tests that rely on Ethereum JSON-RPC.

Use the command to run each group separately:

```
cargo test [tests_nodep|tests_with_db|tests_with_jsonrpc]
```
