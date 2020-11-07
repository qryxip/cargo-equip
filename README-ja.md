# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![dependency status](https://deps.rs/repo/github/qryxip/cargo-equip/status.svg)](https://deps.rs/repo/github/qryxip/cargo-equip)
[![Crates.io](https://img.shields.io/crates/v/cargo-equip.svg)](https://crates.io/crates/cargo-equip)
[![Crates.io](https://img.shields.io/crates/l/cargo-equip.svg)](https://crates.io/crates/cargo-equip)

[English](https://github.com/qryxip/cargo-equip)

競技プログラミング用にRustコードを一つの`.rs`ファイルにバンドルするCargoサブコマンドです。

## 例

[Sqrt Mod - Library-Cheker](https://judge.yosupo.jp/problem/sqrt_mod)

```toml
[package]
name = "solve"
version = "0.0.0"
edition = "2018"

[dependencies]
acl_convolution = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_dsu         = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_fenwicktree = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_lazysegtree = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_math        = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_maxflow     = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_mincostflow = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_modint      = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_scc         = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_segtree     = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_string      = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_twosat      = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
input           = { path = "/path/to/input"                                                                }
output          = { path = "/path/to/output"                                                               }
tonelli_shanks  = { path = "/path/to/tonelli_shanks"                                                       }
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
     Running `/home/ryo/.cargo/bin/rustup run nightly cargo udeps --output json -p solve --bin solve`
    Checking solve v0.0.0 (/home/ryo/src/local/a/solve)
    Finished dev [unoptimized + debuginfo] target(s) in 0.12s
info: Loading save analysis from "/home/ryo/src/local/a/solve/target/debug/deps/save-analysis/solve-f226dae584a15e07.json"
    Bundling the code
    Checking cargo-equip-check-output-oyinvf7zhepdh311 v0.1.0 (/tmp/cargo-equip-check-output-oyinvf7zhepdh311)
    Finished dev [unoptimized + debuginfo] target(s) in 0.39s
```

[Submit Info #29067 - Library-Checker](https://judge.yosupo.jp/submission/29067)

## インストール

`nightly`ツールチェインと[cargo-udeps](https://github.com/est31/cargo-udeps)もインストールしてください。

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

[バイナリでの提供](https://github.com/qryxip/cargo-equip/releases)もしています。

## 使い方

`cargo-equip`で展開できるライブラリには以下の制約があります。

1. 各`lib`には`#[macro_export]`したマクロと同名なアイテムが存在しないようにする。

    cargo-equipは`mod lib_name`直下に`pub use crate::{ それらの名前 }`を挿入するため、展開後の`use`で壊れます。
    マクロは`#[macro_use]`で使ってください。

    ```rust
    // in main source code

    #[macro_use]
    extern crate input as _;
    ```

    展開時にはコメントアウトされます。

    ```rust
    // in main source code

    /*#[macro_use]
    extern crate input as _;*/ // `as _`でなければ`use crate::$name;`が挿入される
    ```

2. 共に展開する予定のクレートを使う場合、`extern crate`を宣言してそれを適当な場所にマウントし、そこを相対パスで参照する。

    cargo-equipは`itertools`等のAtCoderやCodinGameで使えるクレートを除いて、
    `extern crate`を`use crate::extern_crate_name_in_main_crate;`に置き換えます。

    誤って直接使わないように`lib` → `lib`の依存においては対象の名前は[リネーム](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml)しておくことを強く推奨します。

    ```rust
    extern crate __another_lib as another_lib;
    ```

    注意点として、バンドル後のコードが2015 editionで実行される場合(AOJ, yukicoder, ~~Library-Checker~~)、相対パスで参照するときは`self::`を付けてください。

    ```rust
    use self::another_lib::foo::Foo;
    ```

3. 可能な限り絶対パスを使わない。

    cargo-equipはpathの`crate`は`crate::extern_crate_name_in_main_crate`に、`pub(crate)`は`pub(in crate::extern_crate_name_in_main_crate)`に置き換えます。

    ただしこの置き換えは必ず上手くいくかどうかがわかりません。
    できる限り`crate::`よりも`self::`と`super::`を使ってください。

    ```diff
    -use crate::foo::Foo;
    +use super::foo::Foo;
    ```

    またマクロ内では`$crate`を使ってください。
    `macro_rules!`内の`$crate`は`$crate::extern_crate_name_in_main_crate`に置き換えられます。

4. 可能な限りライブラリを小さなクレートに分割する。

    cargo-equipは「クレート内のアイテムの依存関係」を調べることはしません。
    出力結果を64KiBに収めるためにはできるだけ小さなクレートに分割してください。

    ```console
    .
    ├── input
    │   ├── Cargo.lock
    │   ├── Cargo.toml
    │   └── src
    │       └── lib.rs
    ├── output
    │   ├── Cargo.lock
    │   ├── Cargo.toml
    │   └── src
    │       └── lib.rs
    ⋮
    ```

ライブラリが用意できたら、それらを`bin`側の`Cargo.toml`の`[dependencies]`に加えてください。
コンテスト毎にツールでパッケージを自動生成しているならそれのテンプレートに加えてください。

[ac-library-rs](https://github.com/rust-lang-ja/ac-library-rs)を使いたい場合はこれらを使ってください。

```toml
[dependencies]
acl_convolution = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_dsu         = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_fenwicktree = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_lazysegtree = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_math        = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_maxflow     = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_mincostflow = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_modint      = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_scc         = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_segtree     = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_string      = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
acl_twosat      = { git = "https://github.com/qryxip/ac-library-rs", branch = "split-into-separate-crates" }
```

準備ができたらコードを書いてください。
`bin`側の制約は以下の2つです。

1. マクロは`use`しない。qualified pathで使うか`#[macro_use]`で使う。
2. `bin`内に`mod`を作る場合、その中では[Extern Prelude](https://doc.rust-lang.org/reference/items/extern-crates.html#extern-prelude)から名前を解決しない。

```rust
// Uncomment this line if you don't use your libraries. (`--check` still works)
//#![cfg_attr(cargo_equip, cargo_equip::skip)]

#[macro_use]
extern crate input as _;

use std::io::Write as _;

fn main() {
    input! {
        n: usize,
    }

    output::buf_print(|out| {
        macro_rules! println(($($tt:tt)*) => (writeln!(out, $($tt)*).unwrap()));
        for i in 1..=n {
            match i % 15 {
                0 => println!("Fizz Buzz"),
                3 | 6 | 9 | 12 => println!("Fizz"),
                5 | 10 => println!("Buzz"),
                _ => println!("{}", i),
            }
        }
    });
}
```

コードが書けたら`cargo equip`で展開します。
`--bin {binの名前}`か`--src {binのファイルパス}`で`bin`を指定してください。
パッケージ内の`bin`が一つの場合は省略できます。
ただし`default-run`には未対応です。

```console
❯ cargo equip --bin "$name"
```

コードはこのように展開されます。
`extern_crate_name`が`bin`側から与えられていないクレートは`"__internal_lib_0_1_0" + &"_".repeat(n)`のような名前が与えられます。

```diff
+//! # Bundled libraries
+//!
+//! ## `input` (private)
+//!
+//! ### `extern_crate_name`
+//!
+//! `input`
+//!
+//! ## `output` (private)
+//!
+//! ### `extern_crate_name`
+//!
+//! `output`

// Uncomment this line if you don't use your libraries. (`--check` still works)
//#![cfg_attr(cargo_equip, cargo_equip::skip)]

-#[macro_use]
-extern crate input as _;
+/*#[macro_use]
+extern crate input as _;*/

 use std::io::Write as _;

 fn main() {
     input! {
         n: usize,
     }

     output::buf_print(|out| {
         macro_rules! println(($($tt:tt)*) => (writeln!(out, $($tt)*).unwrap()));
         for i in 1..=n {
             match i % 15 {
                 0 => println!("Fizz Buzz"),
                 3 | 6 | 9 | 12 => println!("Fizz"),
                 5 | 10 => println!("Buzz"),
                 _ => println!("{}", i),
             }
         }
     });
 }
+
+// The following code was expanded by `cargo-equip`.
+
+#[allow(dead_code)]
+mod input {
+    // ...
+}
+
+#[allow(dead_code)]
+mod output {
+    // ...
+}
```

cargo-equipがやる操作は以下の通りです。

- `bin`側
    - トップに`#![cfg_attr(cargo_equip, cargo_equip::skip)]`を発見した場合、以下の処理をスキップして`--check`の処理だけ行い出力
    - (もしあるのなら)`mod $name;`をすべて再帰的に展開する。このとき各モジュールをインデントする。ただし複数行にまたがるリテラルが無い場合はインデントしない
    - `extern crate`を処理
    - doc commentを上部に追加
    - 展開した`lib`を下部に追加
- `lib`側
    - `mod $name;`をすべて再帰的に展開する
    - 各パスの`crate`を処理
    - `extern crate`を処理
    - `macro_rules!`を処理
    - `--resolve-cfg`オプションを付けた場合、`#[cfg(常にTRUEのように見える式)]`のアトリビュートと`#[cfg(常にFALSEのように見える式)]`のアトリビュートが付いたアイテムを消去
    - `--remove docs`オプションを付けた場合、doc commentを消去
    - `--remove comments`オプションを付けた場合、commentを消去
- 両方
    - `--minify all`オプションを付けた場合コード全体を最小化する
    - `--rustfmt`オプションを付けた場合Rustfmtでフォーマットする

## オプション

### `--resolve-cfgs`

1. `#[cfg(常にTRUEのように見える式)]` (e.g. `cfg(feature = "enabled-feature")`)のアトリビュートを消去します。
2. `#[cfg(常にFALSEのように見える式)]` (e.g. `cfg(test)`, `cfg(feature = "disable-feature")`)のアトリビュートが付いたアイテムを消去します。

```rust
#[allow(dead_code)]
pub mod a {
    pub struct A;

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
#[allow(dead_code)]
pub mod a {
    pub struct A;
}
```

### `--remove <REMOVE>...`

1. `--remove docs`でDoc comment (`//! ..`, `/// ..`, `/** .. */`, `#[doc = ".."]`)を
2. `--remove comments`でコメント (`// ..`, `/* .. */`)を

除去します。

```rust
#[allow(dead_code)]
pub mod a {
    //! A.

    /// A.
    pub struct A; // aaaaa
}
```

↓

```rust
#[allow(dead_code)]
pub mod a {
    pub struct A;
}
```

### `--minify <MINIFY>`

`--minify lib`で展開後のライブラリをそれぞれ一行に折り畳みます。
`--minify all`でコード全体を最小化します。

ただ現段階では実装が適当なのでいくつか余計なスペースが挟まる場合があります。

### `--rustfmt`

出力をRustfmtでフォーマットします。

### `--check`

バンドルしたコードを出力する前にtarget directoryを共有した一時パッケージを作り、それの上で`cargo check`します。

`#![cfg_attr(cargo_equip, cargo_equip::skip)]`でスキップした場合もチェックします。

```console
❯ cargo equip --check -o /dev/null
     Running `/home/ryo/.cargo/bin/rustup run nightly cargo udeps --output json -p solve --bin solve`
    Checking solve v0.0.0 (/home/ryo/src/local/a/solve)
    Finished dev [unoptimized + debuginfo] target(s) in 0.13s
info: Loading save analysis from "/home/ryo/src/local/a/solve/target/debug/deps/save-analysis/solve-4eea33c8603d6001.json"
    Bundling the code
    Checking cargo-equip-check-output-6j2i3j3tgtugeaqm v0.1.0 (/tmp/cargo-equip-check-output-6j2i3j3tgtugeaqm)
    Finished dev [unoptimized + debuginfo] target(s) in 0.11s
```

## ライセンス

[MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)のデュアルライセンスです。
