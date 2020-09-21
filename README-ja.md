# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Crates.io](https://img.shields.io/crates/v/cargo-equip.svg)](https://crates.io/crates/cargo-equip)
[![Crates.io](https://img.shields.io/crates/l/cargo-equip.svg)](https://crates.io/crates/cargo-equip)

[English](https://github.com/qryxip/cargo-equip)

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
__lib = { package = "lib", path = "/path/to/lib" }
```

```rust
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__lib::{fenwick::AdditiveFenwickTree, input, output};

use std::io::Write as _;

fn main() {
    input! {
        n: usize,
        q: usize,
        r#as: [i64; n],
    }

    let mut fenwick = AdditiveFenwickTree::new(n);

    for (i, a) in r#as.into_iter().enumerate() {
        fenwick.plus(i, &a);
    }

    output::buf_print(|out| {
        macro_rules! println(($($tt:tt)*) => (writeln!(out, $($tt)*).unwrap()));
        for _ in 0..q {
            input!(kind: u32);
            match kind {
                0 => {
                    input!(p: usize, x: i64);
                    fenwick.plus(p, &x);
                }
                1 => {
                    input!(l: usize, r: usize);
                    println!("{}", fenwick.query(l..r));
                }
                _ => unreachable!(),
            }
        }
    });
}
```

↓

```console
$ cargo equip --minify mods --rustfmt --check -o ./bundled.rs
    Bundling code
    Checking cargo-equip-check-output-dsznj7zzfki6wfpq v0.1.0 (/tmp/cargo-equip-check-output-dsznj7zzfki6wfpq)
    Finished dev [unoptimized + debuginfo] target(s) in 0.19s
```

<https://judge.yosupo.jp/submission/23733>

## インストール

### Crates.io

```console
$ cargo install cargo-equip
```

### `master`

```console
$ cargo install --git https://github.com/qryxip/cargo-equip
```

### GitHub Releases

[バイナリでの提供](https://github.com/qryxip/cargo-equip/releases)もしています。

## 使い方

`cargo-equip`で展開できるライブラリには以下5つの制限があります。

1. 非inline module (`mod $name;`)は深さ1まで
2. 深さ2以上のモジュールはすべてinline module (`mod $name { .. }`)
3. crate root直下には`mod`以外の`pub`なアイテムが置かれていない (置いてもいいですが使わないでください)
4. `#[macro_export] macro_rules! name { .. }`は`mod name`の中に置かれている (それ以外の場所に置いていいですがその場合`#[macro_use]`で使ってください)
5. `#[macro_export]`には組み込み以外のアトリビュート(e.g. `#[rustfmt::skip]`)を使用しない (原理的に展開すると壊れる)

1.と2.はそのうち対応しようと思います。

このように薄く広く作ってください。

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

この制限に合うようなライブラリを書いたら、その`Cargo.toml`の`package.metadata`にモジュールの依存関係を手で書いてください。
直接`use`したモジュールと、その連結成分だけを展開します。
欠けている場合はwarningと共にすべてのモジュールを展開します。

[使う側で指定できるようにすることも考えています](https://github.com/qryxip/cargo-equip/issues/2)。

```toml
[package.metadata.cargo-equip-lib.mod-dependencies]
"a" = []
"b" = ["a"]
"c" = ["a"]
```

そして`bin`側の準備として、バンドルしたいライブラリを`dependencies`に加えてください。
コンテスト毎にツールでパッケージを自動生成しているならそれのテンプレートに加えてください。

ただしこの際、ライブラリは誤って直接使わないようにリネームしておくことを強く推奨します。
直接使った場合`cargo-equip`はそれについて何も操作しません。

```toml
[dependencies]
__my_lib = { package = "my_lib", path = "/path/to/my_lib" }
```

準備ができたらこのようにattribute付きでライブラリを`use`します。

```rust
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{b::B, c::C};

// or
//
//#[cfg_attr(cargo_equip, cargo_equip::equip)]
//use ::{
//    __my_lib1::{b::B, c::C},
//    __my_lib2::{d::D, e::E},
//};
```

`use`のパスにはleading colon (`::`)を付けてください。

```
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{b::B, c::C};
    ^^
```

パスの1つ目のsegmentから展開するべきライブラリを決定します。
leading colonを必須としているのはこのためです。

```
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{b::B, c::C};
      ^^^^^^^^
```

先述したライブラリの制約より、パスの第二セグメントはモジュールとみなします。
これらのモジュールと、先程書いた`mod-dependencies`で繋がっているモジュールが展開されます。

```
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{b::B, c::C};
                 ^     ^
```

第三セグメント以降は`use self::$name::{..}`と展開されます。

```
#[cfg_attr(cargo_equip, cargo_equip::equip)]
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
//! # Bundled libraries
//!
//! ## [`my_lib`]({ a link to Crates.io or the repository })
//!
//! ### Modules
//!
//! - `::__my_lib::a` → `$crate::a`
//! - `::__my_lib::b` → `$crate::b`
//! - `::__my_lib::c` → `$crate::c`

/*#[cfg_attr(cargo_equip, cargo_equip::equip)]
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
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::input;
```

## オプション

### `--remove <REMOVE>...`

1. `--remove test-items`で`#[cfg(test)]`が付いたアイテムを
2. `--remove docs`でDoc comment (`//! ..`, `/// ..`, `/** .. */`, `#[doc = ".."]`)を
3. `--remove comments`でコメント (`// ..`, `/* .. */`)を

除去することができます。

```rust
pub mod a {
    //! A.

    /// A.
    pub struct A; // aaaaa

    #[cfg(test)]
    mod tests {
        #[test]
        fn it_works() {
            assert_eq!(2 + 2, 4);
        }
    }
}
```

↓

```rust
pub mod a {
    pub struct A;
}
```

### `--minify <MINIFY>`

`--minify mods`で展開後の各モジュールをそれぞれ一行に折り畳みます。
`--minify all`でコード全体を最小化します。

### `--rustfmt`

出力をRustfmtでフォーマットします。

### `--check`

バンドルしたコードを出力する前にtarget directoryを共有した一時パッケージを作り、それの上で`cargo check`します。

```console
$ cargo equip --check -o /dev/null
    Bundling code
    Checking cargo-equip-check-output-r3cw9cy0swqb5yac v0.1.0 (/tmp/cargo-equip-check-output-r3cw9cy0swqb5yac)
    Finished dev [unoptimized + debuginfo] target(s) in 0.18s
```

## ライセンス

[MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)のデュアルライセンスです。
