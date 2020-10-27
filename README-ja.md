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

`cargo-equip`で展開できるライブラリには以下の制約があります。

1. `#[macro_export] macro_rules! name { .. }`は`mod name`の中に置かれている

    cargo-equipはmain source file内でどのマクロが使われているのかを調べません。
    この制約を守れば`#[macro_export]`したマクロは展開前も展開後も自然に動作します。

    ```rust
    #![cfg_attr(cargo_equip, cargo_equip::equip)]

    use ::__my_lib::input;

    fn main() {
        input! {
            a: u32,
            b: u32,
            s: String,
        }

        todo!();
    }
    ```

2. マクロにおいて`$crate::`でアイテムのパスを指定している場合、`#[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]`を付ける

    cargo-equipはこのアトリビュートが付いた`macro_rules!`内にあるすべての`$crate`を、`::identifier!`と続く場合のみを除いて`$crate::extern_crate_name_in_the_main_crate`と置き換えます。
    アトリビュートが無い場合、またはアトリビュートをtypoしている場合は一切操作しません。

    ```diff
     #[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]
     #[macro_export]
     macro_rules! input {
         ($($tt:tt)*) => {
    -        let __scanner = $crate::input::Scanner::new();
    +        let __scanner = $crate::extern_crate_name_in_the_main_crate::input::Scanner::new();
             $crate::__input_inner!($($tt)*); // as is
         };
     }
    ```

    ただ1.の制約を守っていて、関連するアイテムがすべて同じモジュールにあるなら無くても動く場合があります。

3. 共に展開する予定のクレートを使う場合、**各モジュールに**`#[cfg_attr(cargo_equip, cargo_equip::use_another_lib)]`を付けた`extern crate`を宣言してマウントし、そこを相対パスで参照する。

    cargo-equipはこのアトリビュートの付いた`extern crate`を`use crate::extern_crate_name_in_main_crate;`に置き換えます。

    誤って直接使わないように対象の名前は[リネーム](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml)しておくことを強く推奨します。

    ```rust
    #[cfg_attr(cargo_equip, cargo_equip::use_another_lib)]
    extern crate __another_lib as another_lib;
    ```

    注意点として、バンドル後のコードが2015 editionで実行される場合(yukicoder, Library-Checker)、相対パスで参照するときは`self::`を付けてください。

    ```rust
    use self::another_lib::foo::Foo;
    ```

4. 可能な限り絶対パスを使わない。

    cargo-equipはpathの`crate`は`crate::extern_crate_name_in_main_crate`に、`pub(crate)`は`pub(in crate::extern_crate_name_in_main_crate)`に置き換えます。

    ただしこの置き換えは必ず上手くいくかどうかがわかりません。
    できる限り`crate::`よりも`self::`と`super::`を使ってください。

    ```diff
    -use crate::foo::Foo;
    +use super::foo::Foo;
    ```

5. 可能な限りinline module (`mod $name;`)は深さ1まで

    後述する`package.metadata`でのモジュール依存関係の記述は今のところトップモジュール単位です。

6. crate root直下には`mod`以外の`pub`なアイテムが置かれていない

    置いてもいいですが使わないでください。
    現在cargo-equipは`lib.rs`直下の`mod`以外のアイテムを、読みますがすべて消し飛ばします。


5.と6.はそのうち対応しようと思います。

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
書いていない場合や欠けている場合はwarningと共にすべてのモジュールを展開します。

```toml
[package.metadata.cargo-equip.module-dependencies]
"crate::a" = []
"crate::b" = ["crate::b"]
"crate::c" = ["::__another_lib::d"]
"::__another_lib::d" = ["::__another_lib::e"]
```

最後にマージされるのでどのモジュールの依存関係を、`bin`側を含めてどのパッケージに記述してもよいです。
[ac-library-rs](https://github.com/rust-lang-ja/ac-library-rs)を使う場合、毎回使用するパッケージに以下のものを加えてください。

```toml
# "extern crate name"が`__aclrs`の場合
[package.metadata.cargo-equip.module-dependencies]
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
```

そして`bin`側の準備として、バンドルしたいライブラリを`Cargo.toml`の`dependencies`に加えてください。

ただしこの際、ライブラリは誤って直接使わないように[リネーム](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml)しておくことを強く推奨します。
直接使った場合`cargo-equip`はそれについて何も操作しません。

コンテスト毎にツールでパッケージを自動生成しているならそれのテンプレートに加えてください。

```toml
[dependencies]
__aclrs = { package = "ac-library-rs", git = "https://github.com/rust-lang-ja/ac-library-rs", branch = "replace-absolute-paths" }
__my_lib1 = { package = "my_lib1", path = "/path/to/my_lib1" }
__my_lib2 = { package = "my_lib2", path = "/path/to/my_lib2" }
```

準備ができたらこのようにattribute付きでライブラリを`use`します。

```rust
#![cfg_attr(cargo_equip, cargo_equip::equip)]

use ::__my_lib1::{a::A, b::B};
use ::__my_lib2::{c::C, d::D};
```

leading colon (`::`)が付いた`use`だけ展開されます。

```
use ::__my_lib1::{a::A, b::B};
    ^^
```

パスの1つ目のsegmentから展開するべきライブラリを決定します。
leading colonを必須としているのはこのためです。

```
use ::__my_lib1::{a::A, b::B};
      ^^^^^^^^^
```

先述したライブラリの制約より、パスの第二セグメントはモジュールとみなします。
これらのモジュールと、先程書いた`module-dependencies`で繋がっているモジュールが展開されます。

```
use ::__my_lib1::{a::A, b::B};
                  ^     ^
```

第三セグメント以降は`use self::$extern_crate_name::$module_name::{..}`と展開されます。

```
use ::__my_lib1::{a::A, b::B};
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
//! ## [`my_lib1`]({ a link to Crates.io or the repository })
//!
//! ### Modules
//!
//! - `::__my_lib1::a`
//! - `::__my_lib1::b`
//!
//! ## [`my_lib2`]({ a link to Crates.io or the repository })
//!
//! ### Modules
//!
//! - `::__my_lib2::c`
//! - `::__my_lib2::d`

/*#![cfg_attr(cargo_equip, cargo_equip::equip)]*/

/*use ::__my_lib1::{a::A, b::B};*/
/*use ::__my_lib2::{c::C, d::D};*/

fn main() {
    todo!();
}

// The following code was expanded by `cargo-equip`.

use self::__my_lib1::{a::A, b::B};
use self::__my_lib2::{c::C, d::D};

#[allow(dead_code)]
pub mod __my_lib {
    mod a {
        // ..
    }

    mod b {
        // ..
    }

    // `module-dependencies`で連結されているモジュールも共に展開される
    mod b_dep {
        // ..
    }
}

#[allow(dead_code)]
pub mod __my_lib {
    mod c {
        // ..
    }

    mod d {
        // ..
    }
}
```

cargo-equipがやる操作は以下の通りです。これ以外は何も行いません。

- `bin`側
    - `#![cfg_attr(cargo_equip, cargo_equip::equip)]`を検知し、コメントアウト
    - `::`で始まる`use`を検知し、コメントアウト
    - doc commentを上部に追加
    - 展開した`lib`を下部に追加
- `lib`側
    - `lib.rs`内の`mod`をすべて再帰的に展開する。このとき各モジュールをインデントする。ただし複数行にまたがるリテラルが無い場合はインデントしない
    - `mod`を展開後、`mod`と`extern crate`以外のすべてのトップレベルのアイテムを消去する
    - `#[cfg_attr(cargo_equip, cargo_equip::use_another_lib)]`が付いた`extern crate`を操作
    - `#[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]`が付いた`macro_rules!`を操作
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

```console
$ cargo equip --check -o /dev/null
    Bundling code
    Checking cargo-equip-check-output-r3cw9cy0swqb5yac v0.1.0 (/tmp/cargo-equip-check-output-r3cw9cy0swqb5yac)
    Finished dev [unoptimized + debuginfo] target(s) in 0.18s
```

## ライセンス

[MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)のデュアルライセンスです。
