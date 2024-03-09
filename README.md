# LibSOFL - Smart contract Ops Fuzzy Library

The library to facilitate smart contract and transaction analysis for EVM-compatible blockchains. 

## Components

The repo is managed as a monorepo and workspaces can be found in `crates` folder.

- `core`: common type definitions and core smart contract execution engine.
- `reth`: providing execution context using `reth` database for `core`.
- `jsonrpc`: providing execution context using `jsonrpc` for `core`.
- `periphery`: high-level semantic operations on smart contracts, including cheatcodes. 
- `utils`: auto-loaded configuration, logging, and various utilities.
- `knowledge`: (WIP) knowledge mining in historical transactions.
- `analysis`: Dynamic taint analysis for transaction execution.

## Usage Examples

- [Replay transactions](./crates/reth/src/blockchain/provider.rs#L397)
- [Cheatcodes: Manipulate ERC20 contracts](./crates/periphery/src/cheatcodes/erc20/dex_lp.rs#L278)
- [Contract code mining and RPC service](./crates/knowledge/code/bin/server/main.rs)
    - Cache Etherscan contract source code and serve many useful RPCs:
      - Contract source code query
      - Mapping bytecode location to contract source code location
      - Contract storage layout query 
- [Contract invocation transaction indexing](./crates/knowledge/index/bin/collect/main.rs)
    - Collect all transactions (including those internal ones) that invoke a contract. 
    - Provide a index such that the query of all invocations on a contract code can be served.