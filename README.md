# Hank for Rust

A Rust implementation of the Hank language.

This repository provides a high-performance, memory-safe Rust library (`hank`) for embedding the Hank interpreter into any Rust application.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
hank = { git = "https://github.com/Igazine/hank-rust.git" }
```

## Features

- **High Performance**: Optimized tree-walking interpreter.
- **AST Caching**: Eliminates parsing overhead for repeated execution.
- **Embedded Friendly**: Minimal resource footprint (tested on ARM Linux).
- **Standard Library**: Full parity with official specifications.

## Example Runner

An example CLI runner is included in `examples/runner`. Note that the runner requires the universal conformance suite located in the `hank` submodule.

To fetch submodules after cloning:

```bash
git submodule update --init --recursive
```

To run the conformance tests:

```bash
cargo run --example runner
```

## Project Links

- **Hank Core Repo**: [Igazine/hank](https://github.com/Igazine/hank)
- **Official Documentation**: [https://igazine.github.io/hank/](https://igazine.github.io/hank/)

## License

This project is licensed under the MIT License.
