# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![dependency status](https://deps.rs/repo/github/qryxip/cargo-equip/status.svg)](https://deps.rs/repo/github/qryxip/cargo-equip)
[![Crates.io](https://img.shields.io/crates/v/cargo-equip.svg)](https://crates.io/crates/cargo-equip)
[![Crates.io](https://img.shields.io/crates/l/cargo-equip.svg)](https://crates.io/crates/cargo-equip)

[日本語](https://github.com/qryxip/cargo-equip/blob/master/README-ja.md)

A Cargo subcommand to bundle your code into one `.rs` file for competitive programming.

## Example

[Sqrt Mod - Library-Cheker](https://judge.yosupo.jp/problem/sqrt_mod)

```toml
[package]
name = "solve"
version = "0.0.0"
edition = "2018"

[dependencies]
ac-library-rs-parted-convolution = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-dsu         = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-fenwicktree = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-lazysegtree = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-math        = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-maxflow     = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-mincostflow = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-modint      = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-scc         = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-segtree     = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-string      = { git = "https://github.com/qryxip/ac-library-rs-parted" }
ac-library-rs-parted-twosat      = { git = "https://github.com/qryxip/ac-library-rs-parted" }
input                            = { path = "/path/to/input"                                }
output                           = { path = "/path/to/output"                               }
tonelli_shanks                   = { path = "/path/to/tonelli_shanks"                       }
# ...
```

```rust
// Uncomment this line if you don't use your libraries. (`--check` still works)
//#![cfg_attr(cargo_equip, cargo_equip::skip)]

#[macro_use]
extern crate input as _;

use acl_modint::ModInt;
use std::io::Write as _;
use tonelli_shanks::ModIntBaseExt as _;

use permutohedron as _;

fn main() {
    input! {
        yps: [(u32, u32)],
    }

    output::buf_print(|out| {
        macro_rules! println(($($tt:tt)*) => (writeln!(out, $($tt)*).unwrap()));
        for (y, p) in yps {
            ModInt::set_modulus(p);
            if let Some(sqrt) = ModInt::new(y).sqrt() {
                println!("{}", sqrt);
            } else {
                println!("-1");
            }
        }
    });
}
```

↓

```console
❯ cargo equip --resolve-cfgs --remove docs --minify libs --rustfmt --check -o ./bundled.rs
     Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo check --message-format json -p -p 'ac-library-rs-parted-build:0.1.0' -p 'ac-library-rs-parted-convolution:0.1.0' -p 'ac-library-rs-parted-dsu:0.1.0' -p 'ac-library-rs-parted-fenwicktree:0.1.0' -p 'ac-library-rs-parted-internal-bit:0.1.0' -p 'ac-library-rs-parted-internal-math:0.1.0' -p 'ac-library-rs-parted-internal-queue:0.1.0' -p 'ac-library-rs-parted-internal-scc:0.1.0' -p 'ac-library-rs-parted-internal-type-traits:0.1.0' -p 'ac-library-rs-parted-lazysegtree:0.1.0' -p 'ac-library-rs-parted-math:0.1.0' -p 'ac-library-rs-parted-maxflow:0.1.0' -p 'ac-library-rs-parted-mincostflow:0.1.0' -p 'ac-library-rs-parted-modint:0.1.0' -p 'ac-library-rs-parted-scc:0.1.0' -p 'ac-library-rs-parted-segtree:0.1.0' -p 'ac-library-rs-parted-string:0.1.0' -p 'ac-library-rs-parted-twosat:0.1.0' -p 'anyhow:1.0.34' -p 'byteorder:1.3.4' -p 'num-traits:0.2.14' -p 'proc-macro2:1.0.10' -p 'ryu:1.0.5' -p 'serde:1.0.113' -p 'serde_derive:1.0.113' -p 'serde_json:1.0.59' -p 'syn:1.0.17' -p 'typenum:1.12.0'`
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running `/home/ryo/.cargo/bin/rustup run nightly cargo udeps --output json -p solve --bin solve`
    Checking solve v0.0.0 (/home/ryo/src/local/a/solve)
    Finished dev [unoptimized + debuginfo] target(s) in 0.16s
info: Loading save analysis from "/home/ryo/src/local/a/solve/target/debug/deps/save-analysis/solve-2970d6e10b9c0877.json"
    Bundling the code
    Checking cargo-equip-check-output-nq4nm7zkj9vtgbd9 v0.1.0 (/tmp/cargo-equip-check-output-nq4nm7zkj9vtgbd9)
    Finished dev [unoptimized + debuginfo] target(s) in 0.35s
```

[Submit Info #29083 - Library-Checker](https://judge.yosupo.jp/submission/29083)

## Installation

Install a `nightly` toolchain and [cargo-udeps](https://github.com/est31/cargo-udeps) first.

```console
❯ rustup update nightly
```

```console
❯ cargo install --git https://github.com/est31/cargo-udeps # for est31/cargo-udeps#80
```

### Crates.io

```console
❯ cargo install cargo-equip
```

### `master`

```console
❯ cargo install --git https://github.com/qryxip/cargo-equip
```

### GitHub Releases

[Releases](https://github.com/qryxip/cargo-equip/releases)

## Usage

TODO ([Japanese](https://github.com/qryxip/cargo-equip/blob/master/README-ja.md#使い方))

## License

Dual-licensed under [MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0).
