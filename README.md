# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Crates.io](https://img.shields.io/crates/v/cargo-equip.svg)](https://crates.io/crates/cargo-equip)
[![Crates.io](https://img.shields.io/crates/l/cargo-equip.svg)](https://crates.io/crates/cargo-equip)

[日本語](https://github.com/qryxip/cargo-equip/blob/master/README-ja.md)

A Cargo subcommand to bundle your code into one `.rs` file for competitive programming.

## Example

[Point Add Range Sum - Library-Cheker](https://judge.yosupo.jp/problem/point_add_range_sum)

`lib`

```toml
[package.metadata.cargo-equip-lib.mod-dependencies]
"algebraic" = []
"fenwick" = ["algebraic"]
"input" = []
"output" = []
```

`bin`

```toml
[dependencies]
__complib = { package = "complib", path = "/path/to/complib" }
cargo_equip_marker = "0.1.1"
```

```rust
#[cargo_equip::equip]
use ::__complib::{fenwick::AdditiveFenwickTree, input, output};

use std::io::Write as _;

fn main() {
    input! {
        n: usize,
        q: usize,
        r#as: [i64; n],
    }

    let mut bit = AdditiveFenwickTree::new(n);

    for (i, a) in r#as.into_iter().enumerate() {
        bit.plus(i, &a);
    }

    output::buf_print(|out| {
        macro_rules! println(($($tt:tt)*) => (std::writeln!(out, $($tt)*).unwrap()));
        for _ in 0..q {
            input!(kind: u32);
            match kind {
                0 => {
                    input!(p: usize, x: i64);
                    bit.plus(p, &x);
                }
                1 => {
                    input!(l: usize, r: usize);
                    println!("{}", bit.query(l..r));
                }
                _ => unreachable!(),
            }
        }
    });
}
```

↓

```console
$ cargo equip --oneline mods --rustfmt --check | xsel -ib
    Bundling code
    Checking cargo-equip-check-output-ixtp05p7mhbiumzg v0.1.0 (/tmp/cargo-equip-check-output-ixtp05p7mhbiumzg)
    Finished dev [unoptimized + debuginfo] target(s) in 0.21s
```

<https://judge.yosupo.jp/submission/20767>

## License

Dual-licensed under [MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0).
