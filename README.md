# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Crates.io](https://img.shields.io/crates/v/cargo-equip.svg)](https://crates.io/crates/cargo-equip)
[![Crates.io](https://img.shields.io/crates/l/cargo-equip.svg)](https://crates.io/crates/cargo-equip)

[日本語](https://github.com/qryxip/cargo-equip/blob/master/README-ja.md)

A Cargo subcommand to bundle your code into one `.rs` file for competitive programming.

## Example

[Sqrt Mod - Library-Cheker](https://judge.yosupo.jp/problem/sqrt_mod)

```toml
[package]
name = "lib"
version = "0.0.0"
edition = "2018"

[package.metadata.cargo-equip.module-dependencies]
"crate::input" = []
"crate::output" = []
"crate::tonelli_shanks" = ["crate::xorshift", "::__atcoder::modint"]
"crate::xorshift" = []
# ..
"::__atcoder::convolution" = ["::__atcoder::internal_bit", "::__atcoder::modint"]
"::__atcoder::internal_bit" = []
"::__atcoder::internal_math" = []
"::__atcoder::internal_queue" = []
"::__atcoder::internal_scc" = []
"::__atcoder::internal_type_traits" = []
"::__atcoder::lazysegtree" = ["::__atcoder::internal_bit", "::__atcoder::segtree"]
"::__atcoder::math" = ["::__atcoder::internal_math"]
"::__atcoder::maxflow" = ["::__atcoder::internal_type_traits", "::__atcoder::internal_queue"]
"::__atcoder::mincostflow" = ["::__atcoder::internal_type_traits"]
"::__atcoder::modint" = ["::__atcoder::internal_math"]
"::__atcoder::scc" = ["::__atcoder::internal_scc"]
"::__atcoder::segtree" = ["::__atcoder::internal_bit", "::__atcoder::internal_type_traits"]
"::__atcoder::twosat" = ["::__atcoder::internal_scc"]

[dependencies]
__atcoder = { package = "ac-library-rs", git = "https://github.com/rust-lang-ja/ac-library-rs", branch = "replace-absolute-paths" }
```

```toml
[package]
name = "bin"
version = "0.0.0"
edition = "2018"

[dependencies]
__atcoder = { package = "ac-library-rs", git = "https://github.com/rust-lang-ja/ac-library-rs", branch = "replace-absolute-paths" }
__lib = { package = "lib", path = "/path/to/lib" }
```

```rust
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::{
    __atcoder::modint::ModInt,
    __lib::{input, output, tonelli_shanks::ModIntBaseExt as _},
};

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
$ cargo equip --remove docs test-items --minify mods --rustfmt --check -o ./bundled.rs
    Bundling code
warning: could not minify the code. inserting spaces: `internal_math`
    Checking cargo-equip-check-output-rml3nu3kghlx3ar4 v0.1.0 (/tmp/cargo-equip-check-output-rml3nu3kghlx3ar4)
    Finished dev [unoptimized + debuginfo] target(s) in 0.30s
```

[Submit Info #24741 - Library-Checker](https://judge.yosupo.jp/submission/24741)

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
