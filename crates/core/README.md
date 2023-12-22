# libsofl-core

`libsofl-core` is the core of `libsofl`, which is desinged to contain the core functionality agnostic to different EVM-compatible blockchians.

## Modules

### Engine

Module `engine`` wraps the `revm` with high level transaction execution APIs on top of a blockchain state trait.
The rationale behind `engine` module is to simplify the usage of `revm` package and provide convenient interface of blockchian state forking and execution.