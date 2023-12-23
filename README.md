# LibSOFL - Smart contract Ops Fuzzy Library

The library to facilitate smart contract and transaction analysis for EVM-compatible blockchains. 

## Components

The repo is managed as a monorepo and workspaces can be found in `crates` folder.

- `core`: common type definitions and core smart contract execution engine.
- `reth`: providing execution context using `reth` database for `core`.
- `jsonrpc`: providing execution context using `jsonrpc` for `core`.
- `periphery`: high-level semantic operations on smart contracts, including cheatcodes. 
- `utils`: auto-loaded configuration and logging.
- `fuzzy`: (WIP) for smart contract fuzzing.
- `knowledge`: (WIP) for historical transaction mining.