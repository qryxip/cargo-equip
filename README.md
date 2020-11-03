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
acl_convolution          = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_dsu                  = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_fenwicktree          = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_internal_bit         = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_internal_math        = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_internal_queue       = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_internal_scc         = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_internal_type_traits = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_lazysegtree          = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_math                 = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_maxflow              = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_mincostflow          = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_modint               = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_scc                  = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_segtree              = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_string               = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_twosat               = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
input                    = { path = "/path/to/input"                                                                }
output                   = { path = "/path/to/output"                                                               }
tonelli_shanks           = { path = "/path/to/tonelli_shanks"                                                       }
xorshift                 = { path = "/path/to/xorshift"                                                             }
# ...
```

```rust
#[macro_use]
extern crate input as _;

use acl_modint::ModInt;
use std::io::Write as _;
use tonelli_shanks::ModIntBaseExt as _;

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
     Running `/home/ryo/.cargo/bin/rustup run nightly cargo udeps --output json -p solve --bin solve`
    Checking solve v0.0.0 (/home/ryo/src/local/a/solve)
    Finished dev [unoptimized + debuginfo] target(s) in 0.30s
info: Loading save analysis from "/home/ryo/src/local/a/solve/target/debug/deps/save-analysis/solve-4eea33c8603d6001.json"
    Bundling the code
    Checking cargo-equip-check-output-do7dy7m13pba4nbx v0.1.0 (/tmp/cargo-equip-check-output-do7dy7m13pba4nbx)
    Finished dev [unoptimized + debuginfo] target(s) in 0.35s
```

[Submit Info #28576 - Library-Checker](https://judge.yosupo.jp/submission/28576)

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
