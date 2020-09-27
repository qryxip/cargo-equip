# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Crates.io](https://img.shields.io/crates/v/cargo-equip.svg)](https://crates.io/crates/cargo-equip)
[![Crates.io](https://img.shields.io/crates/l/cargo-equip.svg)](https://crates.io/crates/cargo-equip)

[English](https://github.com/qryxip/cargo-equip)

競技プログラミング用にRustコードを一つの`.rs`ファイルにバンドルするCargoサブコマンドです。

## 例

[Sqrt Mod - Library-Cheker](https://judge.yosupo.jp/problem/sqrt_mod)

```toml
[package]
name = "bin"
version = "0.0.0"
authors = ["Ryo Yamashita <qryxip@gmail.com>"]
edition = "2018"
publish = false

[package.metadata.cargo-equip.module-dependencies]
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
"::__lib::input" = []
"::__lib::output" = []
"::__lib::tonelli_shanks" = ["::__lib::xorshift"]
"::__lib::xorshift" = []
# ..

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

`cargo-equip`で展開できるライブラリには以下の制限があります。

1. 絶対パスを使わない。クレート内のアイテムはすべて相対パスで書く。

    Rustのパス解決をシミュレートするのは非常に困難であるため、cargo-equipはパスの置き換えを行いません。
    `crate::`は`self::`と`super::`で書き直してください。

    ```diff
    -use crate::foo::Foo;
    +use super::foo::Foo;
    ```

2. 共に展開する予定のクレートを使う場合、各モジュールに`#[cfg_attr(cargo_equip, cargo_equip::use_another_lib)]`を付けた`extern crate`を宣言してマウントし、そこを相対パスで参照する。

    cargo-equipはこのアトリビュートの付いた`extern crate`を`use crate::extern_crate_name_in_main_crate;`に置き換えます。

    誤って直接使わないように対象の名前は[リネーム](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml)しておくことを強く推奨します。

    ```rust
    #[cfg_attr(cargo_equip, cargo_equip::use_another_lib)]
    extern crate __another_lib as another_lib;
    ```

    注意点として、バンドル後のコードが2015 editionで実行される場合(yukicoder, Library-Checker)、相対パスで参照するときは`self::`を付けてください。

    ```diff
    -use another_lib::Foo;
    +use self::another_lib::Foo;
    ```

3. `#[macro_export] macro_rules! name { .. }`は`mod name`の中に置かれている

    cargo-equipはmain source file内でどのマクロが使われているのかを調べません。
    この制約を守れば`#[macro_export]`したマクロは展開前も展開後も自然に動作します。

    ```rust
    #[cfg_attr(cargo_equip, cargo_equip::equip)]
    use ::__my_lib::input;
    ```

4. マクロにおいて`$crate::`でアイテムのパスを指定している場合、`#[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]`を付ける

    cargo-equipはこのアトリビュートが付いた`macro_rules!`内にあるすべての`$crate`を、`::identifier!`と続く場合のみを除いて`$crate::extern_crate_name`と置き換えます。
    アトリビュートが無い場合、またはアトリビュートをtypoしている場合は一切操作しません。
    4.の制約を守っているなら無くても動く場合があります。

    ```rust
    #[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]
    #[macro_export]
    macro_rules! input {
        () => {
            // ..
        };
    }
    ```

5. 非inline module (`mod $name;`)は深さ1まで

6. 深さ2以上のモジュールはすべてinline module (`mod $name { .. }`)

7. crate root直下には`mod`以外の`pub`なアイテムが置かれていない

    置いてもいいですが使わないでください。
    現在cargo-equipは`lib.rs`直下の`mod`以外のアイテムをすべて無視します。


5.と6.と7.はそのうち対応しようと思います。

このように薄く広く作ってください。
ディレクトリに分けたくなったらクレートを分割してください。

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

そして`bin`側の準備として、バンドルしたいライブラリを`Cargo.toml`の`dependencies`に加えてください。
コンテスト毎にツールでパッケージを自動生成しているならそれのテンプレートに加えてください。

ただしこの際、ライブラリは誤って直接使わないように[リネーム](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml)しておくことを強く推奨します。
直接使った場合`cargo-equip`はそれについて何も操作しません。

```toml
[dependencies]
__atcoder = { package = "ac-library-rs", git = "https://github.com/rust-lang-ja/ac-library-rs", branch = "replace-absolute-paths" }
__my_lib = { package = "my_lib", path = "/path/to/my_lib" }
```

この制限に合うようなライブラリを書いたら、その`Cargo.toml`の`package.metadata`にモジュールの依存関係を手で書いてください。
直接`use`したモジュールと、その連結成分だけを展開します。
書いていない場合や欠けている場合はwarningと共にすべてのモジュールを展開します。

```toml
[package.metadata.cargo-equip.module-dependencies]
"::__my_lib::a" = ["::__my_lib::c"]
"::__my_lib::b" = []
"::__my_lib::c" = []
```

準備ができたらこのようにattribute付きでライブラリを`use`します。

```rust
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{a::A, b::B};

// or
//
//#[cfg_attr(cargo_equip, cargo_equip::equip)]
//use ::{
//    __my_lib1::{a::A, b::B},
//    __my_lib2::{c::C, c::C},
//};
```

`use`のパスにはleading colon (`::`)を付けてください。

```
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{a::A, b::B};
    ^^
```

パスの1つ目のsegmentから展開するべきライブラリを決定します。
leading colonを必須としているのはこのためです。

```
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{a::A, b::B};
      ^^^^^^^^
```

先述したライブラリの制約より、パスの第二セグメントはモジュールとみなします。
これらのモジュールと、先程書いた`module-dependencies`で繋がっているモジュールが展開されます。

```
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{a::A, b::B};
                 ^     ^
```

第三セグメント以降は`use self::$extern_crate_name::$module_name::{..}`と展開されます。

```
#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{a::A, b::B};
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
//! - `::__my_lib::a`
//! - `::__my_lib::b`
//! - `::__my_lib::c`

/*#[cfg_attr(cargo_equip, cargo_equip::equip)]
use ::__my_lib::{a::A, b::B};*/

fn main() {
    todo!();
}

// The following code was expanded by `cargo-equip`.

use self::__my_lib::a::A;
use self::__my_lib::b::B;

#[allow(dead_code)]
pub mod __my_lib {
    mod a {
        // ..
    }

    mod b {
        // ..
    }

    // `a`で使われていると`mod-dependencies`に記述されているため、展開される
    mod c {
        // ..
    }
}
```

cargo-equipがやる操作は以下の通りです。これ以外は何も行いません。

- `bin`側
    - `cargo_equip::equip`が付いた`use`をコメントアウト
    - doc commentを上部に追加
    - 展開した`lib`を下部に追加
- `lib`側
    - 各モジュールの中身をインデントする。ただし複数行にまたがるリテラルが無い場合はそのまま
    - `#[cfg_attr(cargo_equip, cargo_equip::use_another_lib)]`が付いた`extern crate`を操作
    - `#[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]`が付いた`macro_rules!`を操作
    - `--remove <REMOVE>...`オプションを付けた場合対象を操作
- 両方
    - `--minify all`オプションを付けた場合コード全体を最小化する
    - `--rustfmt`オプションを付けた場合Rustfmtでフォーマットする

## オプション

### `--remove <REMOVE>...`

1. `--remove test-items`で`#[cfg(test)]`が付いたアイテムを
2. `--remove docs`でDoc comment (`//! ..`, `/// ..`, `/** .. */`, `#[doc = ".."]`)を
3. `--remove comments`でコメント (`// ..`, `/* .. */`)を

除去することができます。

```rust
#[allow(dead_code)]
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
#[allow(dead_code)]
pub mod a {
    pub struct A;
}
```

### `--minify <MINIFY>`

`--minify mods`で展開後の各モジュールをそれぞれ一行に折り畳みます。
`--minify all`でコード全体を最小化します。

ただ現段階では実装が適当なのでいくつか余計なスペースが挟まる場合があります。

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
