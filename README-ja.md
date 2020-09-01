# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

[日本語](https://github.com/qryxip/cargo-equip/blob/master/README-ja.md)

競技プログラミング用にRustコードを一つの`.rs`ファイルにバンドルするCargoサブコマンドです。

## 例

[Point Add Range Sum - Library-Cheker](https://judge.yosupo.jp/problem/point_add_range_sum)

`lib`側

```toml
[package.metadata.cargo-equip-lib.mod-dependencies]
"algebraic" = []
"fenwick" = ["algebraic"]
"input" = []
"output" = []
```

`bin`側

```toml
[dependencies]
__complib = { package = "complib", path = "/path/to/complib" }
cargo_equip_marker = { git = "https://github.com/qryxip/cargo-equip", rev = "37d4972d57be0d41d3d8edfb5db691487359cb3b" }
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

## インストール

### Crates.io

まだです。

### `master`

```console
$ cargo install --git https://github.com/qryxip/cargo-equip
```

### GitHub Releases

まだです。

## 使い方

まずライブラリをこのように横に広く作ってください。
深さ2以上のモジュールはinline module (`mod { .. }`)として書いてください。

```
src
├── a.rs
├── b.rs
├── c.rs
└── lib.rs
```

```rust
// src/lib.rs
pub mod a;
pub mod b;
pub mod c;
```

次にライブラリの`Cargo.toml`の`package.metadata`にモジュールの依存関係を手で書いてください。
欠けている場合はwarningと共にすべてのモジュールを展開します。

[使う側で指定できるようにすることも考えています](https://github.com/qryxip/cargo-equip/issues/2)。

```toml
[package.metadata.cargo-equip-lib.mod-dependencies]
"a" = []
"b" = ["a"]
"c" = ["a"]
```

`bin`側の準備としては、次の2つを`dependencies`に加えてください。

- 展開したいライブラリ
- `cargo_equip_marker`

```toml
[dependencies]
__my_lib = { package = "my_lib", path = "/path/to/my_lib" }
cargo_equip_marker = { git = "https://github.com/qryxip/cargo-equip", rev = "37d4972d57be0d41d3d8edfb5db691487359cb3b" }
```

ライブラリは誤って直接使わないようにリネームしておくことを強く推奨します。

`cargo_equip_marker`は何もしないattribute macroである`#[equip]`を提供します。
[`cargo-snippet`](https://github.com/hatoo/cargo-snippet)のやりかたを真似たものですが、本ツールでは別のパッケージに分けています。
`cargo-equip`と間違えないようにしてください。
`cargo_equip_marker`から`#[::cargo_equip::equip]`として呼びます。

準備ができたらこのようにライブラリを`use`します。

```rust
#[cargo_equip::equip]
use ::__my_lib::{b::B, c::C};
```

`use`のパスにはleading colon (`::`)を付けてください。

```
#[cargo_equip::equip]
use ::__my_lib::{b::B, c::C};
    ^^
```

パスの1つ目のsegmentから展開するべきライブラリを決定します。
leading colonを必須としているのはこのためです。

```
#[cargo_equip::equip]
use ::__my_lib::{b::B, c::C};
      ^^^^^^^^
```

パスの2つ目のsegmentから使用しているモジュールを判定します。
これらのモジュールと、先程書いた`mod-dependencies`で繋がっているモジュールが展開されます。

```
#[cargo_equip::equip]
use ::__my_lib::{b::B, c::C};
                 ^     ^
```

パスの3つ目以降のsegmentは`use self::$name::{ .. }`として展開されます。

```
#[cargo_equip::equip]
use ::__my_lib::{b::B, c::C};
                    ^     ^
```

コードが書けたら`cargo equip`で展開します。
`--bin {binの名前}`か`--src {binのファイルパス}`で`bin`を指定してください。
パッケージ内の`bin`が一つの場合は省略できます。
ただし`default-run`には未対応です。

```console
$ cargo equip --bin "$name"
```

コードはこのように展開されます。

```rust
/*#[cargo_equip::equip]
use ::__my_lib::{b::B, c::C};*/

fn main() {
    todo!();
}

// The following code was expanded by `cargo-equip`.

use self::b::B;
use self::c::C;

// `b`と`c`で使われていると`mod-dependencies`に記述されているため、展開される
mod a {
    // ..
}

mod b {
    // ..
}

mod c {
    // ..
}
```

モジュールの階層が変わらないため、各ファイルの中身を手を加えずにそのまま展開します。
そのため壊れにくくなっているはずです。
多分。

またライブラリ内の`#[macro_export]`しているマクロですが、マクロ名と同名のモジュールに入れておくと自然な形で使えると思います。

```rust
// input.rs

#[macro_export]
macro_rules! input {
    ($($tt:tt)*) => {
        compile_error!("TODO")
    };
}
```

```rust
#[cargo_equip::equip]
use ::__my_lib::input;
```

## オプション

### `--oneline`

`--oneline mods`で展開後の各モジュールをそれぞれ一行に折り畳みます。
`--oneline all`でコード全体を一行に折り畳みます。

トークン列を`" "`区切りで出力しているだけなので、minificationではありません。

### `--rustfmt`

出力をRustfmtでフォーマットします。

### `--check`

バンドルしたコードを出力する前にtarget directoryを共有した一時パッケージを作り、それの上で`cargo check`します。

```console
$ cargo equip --check > /dev/null
    Bundling code
    Checking cargo-equip-check-output-6oxyyu9lsf9s0f6g v0.1.0 (/tmp/cargo-equip-check-output-6oxyyu9lsf9s0f6g)
    Finished dev [unoptimized + debuginfo] target(s) in 0.18s
```

## ライセンス

[MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)のデュアルライセンスです。
