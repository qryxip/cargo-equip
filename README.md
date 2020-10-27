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
name = "my_library"
version = "0.0.0"
edition = "2018"

[package.metadata.cargo-equip.module-dependencies]
"crate::input" = []
"crate::output" = []
"crate::tonelli_shanks" = ["crate::xorshift", "::__aclrs::modint"]
"crate::xorshift" = []
# ..
"::__aclrs::convolution" = ["::__aclrs::internal_bit", "::__aclrs::modint"]
"::__aclrs::internal_bit" = []
"::__aclrs::internal_math" = []
"::__aclrs::internal_queue" = []
"::__aclrs::internal_scc" = []
"::__aclrs::internal_type_traits" = []
"::__aclrs::lazysegtree" = ["::__aclrs::internal_bit", "::__aclrs::segtree"]
"::__aclrs::math" = ["::__aclrs::internal_math"]
"::__aclrs::maxflow" = ["::__aclrs::internal_type_traits", "::__aclrs::internal_queue"]
"::__aclrs::mincostflow" = ["::__aclrs::internal_type_traits"]
"::__aclrs::modint" = ["::__aclrs::internal_math"]
"::__aclrs::scc" = ["::__aclrs::internal_scc"]
"::__aclrs::segtree" = ["::__aclrs::internal_bit", "::__aclrs::internal_type_traits"]
"::__aclrs::twosat" = ["::__aclrs::internal_scc"]

[dependencies]
__aclrs = { package = "ac-library-rs", git = "https://github.com/rust-lang-ja/ac-library-rs", branch = "replace-absolute-paths" }
```

```toml
[package]
name = "solve"
version = "0.0.0"
edition = "2018"

[dependencies]
__aclrs = { package = "ac-library-rs", git = "https://github.com/rust-lang-ja/ac-library-rs", branch = "replace-absolute-paths" }
__my = { package = "my_library", path = "../my_library" }
```

```rust
#![cfg_attr(cargo_equip, cargo_equip::equip)]

use ::__aclrs::modint::ModInt;
use ::__my::{input, output, tonelli_shanks::ModIntBaseExt as _};
use std::io::Write as _;

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
    Bundling the code
warning: found `crate` paths. replacing them with `crate::__aclrs`
    Checking cargo-equip-check-output-nhuj1nqc32ksbrs2 v0.1.0 (/tmp/cargo-equip-check-output-nhuj1nqc32ksbrs2)
    Finished dev [unoptimized + debuginfo] target(s) in 0.31s
```

[Submit Info #27831 - Library-Checker](https://judge.yosupo.jp/submission/27831)

## Installation

### Crates.io

```console
$ cargo install cargo-equip
```

### `master`

```console
$ cargo install --git https://github.com/qryxip/cargo-equip
```

### GitHub Releases

[Releases](https://github.com/qryxip/cargo-equip/releases)

## Usage

TODO ([Japanese](https://github.com/qryxip/cargo-equip/blob/master/README-ja.md#使い方))

## License

Dual-licensed under [MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0).
