# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![dependency status](https://deps.rs/repo/github/qryxip/cargo-equip/status.svg)](https://deps.rs/repo/github/qryxip/cargo-equip)
[![Crates.io](https://img.shields.io/crates/v/cargo-equip.svg)](https://crates.io/crates/cargo-equip)
[![Crates.io](https://img.shields.io/crates/l/cargo-equip.svg)](https://crates.io/crates/cargo-equip)

[English](https://github.com/qryxip/cargo-equip)

競技プログラミング用にRustコードを一つの`.rs`ファイルにバンドルするCargoサブコマンドです。

## 更新情報

更新情報は[CHANGELOG.md](https://github.com/qryxip/cargo-equip/blob/master/CHANGELOG.md)にあります。

## 例

[Sqrt Mod - Library-Cheker](https://judge.yosupo.jp/problem/sqrt_mod)

```toml
[package]
name = "solve"
version = "0.0.0"
edition = "2018"

[dependencies]
ac-library-rs-parted              = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-convolution  = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-dsu          = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-fenwicktree  = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-lazysegtree  = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-math         = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-maxflow      = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-mincostflow  = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-modint       = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-scc          = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-segtree      = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-string       = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
ac-library-rs-parted-twosat       = { git = "https://github.com/qryxip/ac-library-rs-parted"            }
qryxip-competitive-fastout        = { git = "https://github.com/qryxip/competitive-programming-library" }
qryxip-competitive-input          = { git = "https://github.com/qryxip/competitive-programming-library" }
qryxip-competitive-tonelli-shanks = { git = "https://github.com/qryxip/competitive-programming-library" }
# ...
```

```rust
#[macro_use]
extern crate fastout as _;
#[macro_use]
extern crate input as _;

use acl_modint::ModInt;
use tonelli_shanks::ModIntBaseExt as _;

#[fastout]
fn main() {
    input! {
        yps: [(u32, u32)],
    }

    for (y, p) in yps {
        ModInt::set_modulus(p);
        if let Some(sqrt) = ModInt::new(y).sqrt() {
            println!("{}", sqrt);
        } else {
            println!("-1");
        }
    }
}
```

↓

```console
❯ cargo equip --resolve-cfgs --remove comments docs --rustfmt --check --bin solve | xsel -b
```

[Submit Info #40609 - Library-Checker](https://judge.yosupo.jp/submission/40609)

## インストール

`nightly`ツールチェインと[cargo-udeps](https://github.com/est31/cargo-udeps)もインストールしてください。

```console
❯ rustup update nightly
```

```console
❯ cargo install cargo-udeps
```

### Crates.ioから

```console
❯ cargo install cargo-equip
```

### `master`ブランチから

```console
❯ cargo install --git https://github.com/qryxip/cargo-equip
```

### GitHub Releases

[バイナリでの提供](https://github.com/qryxip/cargo-equip/releases)もしています。

## 使い方

`cargo-equip`で展開できるライブラリには以下の制約があります。

1. 各crate rootには`#[macro_export]`したマクロと同名なアイテムが存在しないようにする。

    cargo-equipは`mod lib_name`直下に`pub use crate::{ それらの名前 };`を挿入するため、展開後の`use`で壊れます。
    `bin`側ではマクロは`#[macro_use]`で使ってください。

    ```rust
    // in main source code

    #[macro_use]
    extern crate input as _;
    ```

    `bin`内の`extern crate`はコメントアウトされます。

    ```rust
    // in main source code

    /*#[macro_use]
    extern crate input as _;*/ // `as _`でなければ`use crate::$name;`が挿入される
    ```

2. **Rust 2015に展開する場合のみ**、共に展開する予定のクレートを使うときに[extern prelude](https://doc.rust-lang.org/reference/items/extern-crates.html#extern-prelude)から直接名前を解決しない。

    **ルートモジュール以外のモジュールで**`extern crate`を宣言してマウントし、そこを相対パスで参照してください。

    cargo-equipは`--exclude <SPEC>...`, `--exclude-atcoder-crates`, `--exclude-codingame-crates`で指定されたクレートを除いて、
    `extern crate`を`use crate::extern_crate_name_in_main_crate;`に置き換えます。

    `lib`同士を`exter crate`で参照する場合、誤って直接使わないように対象の名前は[リネーム](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml)しておくことを強く推奨します。

    ```diff
     mod extern_crates {
    -    pub(super) extern crate __another_lib as another_lib;
    +    pub(super) use crate::another_lib;
     }

     use self::extern_crates::another_lib::foo::Foo; // Prepend `self::` to make compatible with Rust 2015
    ```

    AOJ ~~やyukicoder~~ 等のRustが2018が利用できないサイトにこのツールを使用しないなら不要です。

    2018向けにはcargo-equipは各ライブラリにこのような`mod __pseudo_extern_prelude`を作り、extern preludeの代用にします。
    この`mod __pseudo_extern_prelude`自体はRust 2015でもコンパイルできますが、Rust 2015は`use another_lib::A;`を解決できません。

    ```diff
    +mod __pseudo_extern_prelude {
    +    pub(super) use crate::{another_lib1, another_lib2};
    +}
    +use self::__pseudo_extern_prelude::*;
    +
     use another_lib1::A;
     use another_lib2::B;
    ```

3. マクロ内では`crate`ではなく`$crate`を使う。

    `macro_rules!`内の`$crate`は`$crate::extern_crate_name_in_main_crate`に置き換えられます。
    `macro_rules!`内の`crate`は置き換えられません。

4. 3.以外の場合も可能な限り絶対パスを使わない。

    cargo-equipはpathの`crate`は`crate::extern_crate_name_in_main_crate`に、`pub(crate)`は`pub(in crate::extern_crate_name_in_main_crate)`に置き換えます。

    ただしこの置き換えは必ず上手くいくかどうかがわかりません。
    できる限り`crate::`よりも`self::`と`super::`を使ってください。

    ```diff
    -use crate::foo::Foo;
    +use super::foo::Foo;
    ```

5. 可能な限りライブラリを小さなクレートに分割する。

    cargo-equipは「クレート内のアイテムの依存関係」を調べることはしません。
    AtCoder以外に参加する場合は、出力結果を制限内(たいてい64KiB程度)に収めるためにできるだけ小さなクレートに分割してください。

    ```console
    .
    ├── input
    │   ├── Cargo.toml
    │   └── src
    │       └── lib.rs
    ├── output
    │   ├── Cargo.toml
    │   └── src
    │       └── lib.rs
    ⋮
    ```

ライブラリが用意できたら、それらを`bin`側の`Cargo.toml`の`[dependencies]`に加えてください。
コンテスト毎にツールでパッケージを自動生成しているならそれのテンプレートに加えてください。

[rust-lang-ja/ac-library-rs](https://github.com/rust-lang-ja/ac-library-rs)を使いたい場合、[qryxip/ac-library-rs-parted](https://github.com/qryxip/ac-library-rs-parted)を使ってください。

本物のac-library-rsを ~~`custom-build`内で自動で加工する~~ スクリプトで加工したクレートです。
~~`custom-build`部分は[AtCoder環境と同様のCargo.lock](https://github.com/qryxip/cargo-compete/blob/ba8e0e747ed90768d9f50f3061374162dade8450/resources/atcoder-cargo-lock.toml)を壊さないために`syn 1.0.17`と`proc-macro2 1.0.10`で書かれています。~~
やっぱり小さいといってもdependencyが数十個付いてきてCI等で煩わしいのでやめました。
現在のこれらのクレートは外部の依存クレートを持たず、瞬時にビルド可能です。

```toml
[dependencies]
ac-library-rs-parted             = { git = "https://github.com/qryxip/ac-library-rs-parted" }
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
```

準備ができたらコードを書いてください。
`bin`側の制約は以下の2つです。

1. マクロは`use`しない。qualified pathで使うか`#[macro_use]`で使う。
2. `bin`内に`mod`を作る場合、その中では[extern prelude](https://doc.rust-lang.org/reference/items/extern-crates.html#extern-prelude)から展開予定のライブラリの名前を解決しない。

```rust
#[macro_use]
extern crate input as _;

use std::io::Write as _;

fn main() {
    input! {
        n: usize,
    }

    buffered_print::buf_print(|out| {
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
`extern_crate_name`が`bin`側から与えられていないクレートは`__package_name_0_1_0`のような名前が与えられます。

```diff
```diff
+//! # Bundled libraries
+//!
+//! - `qryxip-competitive-buffered-print 0.0.0 (path+█████████████████████████████████████████████████████████████████████████████████████)` published in https://github.com/qryxip/competitive-programming-library licensed under `CC0-1.0` as `crate::buffered_print`
+//! - `qryxip-competitive-input 0.0.0 (path+████████████████████████████████████████████████████████████████████████████)`                   published in https://github.com/qryxip/competitive-programming-library licensed under `CC0-1.0` as `crate::input`

-#[macro_use]
-extern crate input as _;
+/*#[macro_use]
+extern crate input as _;*/

 use std::io::Write as _;

 fn main() {
     input! {
         n: usize,
     }

     buffered_print::buf_print(|out| {
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
+mod buffered_print {
+    // ...
+}
+
+#[allow(dead_code)]
+mod input {
+    // ...
+}
```

cargo-equipがやる操作は以下の通りです。

- `bin`側
    - トップに`#![cfg_attr(cargo_equip, cargo_equip::skip)]`を発見した場合、以下の処理をスキップして`--check`の処理だけ行い出力
    - (もしあるのなら)`mod $name;`をすべて再帰的に展開する。このとき各モジュールをインデントする。ただし複数行にまたがるリテラルが無い場合はインデントしない
    - 手続き型マクロを展開
    - `extern crate`を処理
    - doc commentを上部に追加
    - 展開した`lib`を下部に追加
- `lib`側
    - `mod $name;`をすべて再帰的に展開する
    - 各パスの`crate`を処理
    - `extern crate`を処理
    - `macro_rules!`を処理
    - `mod __pseudo_extern_prelude { .. }`と`use (self::|$(super::)*)__pseudo_extern_prelude::*;`を挿入
    - `--resolve-cfg`オプションを付けた場合、`#[cfg(常にTRUEのように見える式)]`のアトリビュートと`#[cfg(常にFALSEのように見える式)]`のアトリビュートが付いたアイテムを消去
    - `--remove docs`オプションを付けた場合、doc commentを消去
    - `--remove comments`オプションを付けた場合、commentを消去
- 全体
    - `--minify all`オプションを付けた場合コード全体を最小化する
    - `--rustfmt`オプションを付けた場合Rustfmtでフォーマットする

## 手続き型マクロの展開

cargo-equipは手続き型マクロを展開する機能を持っています。

```rust
#[macro_use]
extern crate memoise as _;
#[macro_use]
extern crate proconio_derive as _;

#[fastout]
fn main() {
    for i in 0..=100 {
        println!("{}", fib(i));
    }
}

#[memoise(n <= 100)]
fn fib(n: i64) -> i64 {
    if n == 0 || n == 1 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}
```

↓

<details>
<summary>Output</summary>

```rust
//! # Procedural macros
//!
//! - `memoise 0.3.2 (registry+https://github.com/rust-lang/crates.io-index)`         licensed under `BSD-3-Clause`
//! - `proconio-derive 0.2.1 (registry+https://github.com/rust-lang/crates.io-index)` licensed under `MIT OR Apache-2.0`

/*#[macro_use]
extern crate memoise as _;*/
/*#[macro_use]
extern crate proconio_derive as _;*/

/*#[fastout]
fn main() {
    for i in 0..=100 {
        println!("{}", fib(i));
    }
}*/
fn main() {
    let __proconio_stdout = ::std::io::stdout();
    let mut __proconio_stdout = ::std::io::BufWriter::new(__proconio_stdout.lock());
    #[allow(unused_macros)]
    macro_rules ! print { ($ ($ tt : tt) *) => { { use std :: io :: Write as _ ; :: std :: write ! (__proconio_stdout , $ ($ tt) *) . unwrap () ; } } ; }
    #[allow(unused_macros)]
    macro_rules ! println { ($ ($ tt : tt) *) => { { use std :: io :: Write as _ ; :: std :: writeln ! (__proconio_stdout , $ ($ tt) *) . unwrap () ; } } ; }
    let __proconio_res = {
        for i in 0..=100 {
            println!("{}", fib(i));
        }
    };
    <::std::io::BufWriter<::std::io::StdoutLock> as ::std::io::Write>::flush(
        &mut __proconio_stdout,
    )
    .unwrap();
    return __proconio_res;
}

/*#[memoise(n <= 100)]
fn fib(n: i64) -> i64 {
    if n == 0 || n == 1 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}*/
thread_local ! (static FIB : std :: cell :: RefCell < Vec < Option < i64 > > > = std :: cell :: RefCell :: new (vec ! [None ; 101usize]));
fn fib_reset() {
    FIB.with(|cache| {
        let mut r = cache.borrow_mut();
        for r in r.iter_mut() {
            *r = None
        }
    });
}
fn fib(n: i64) -> i64 {
    if let Some(ret) = FIB.with(|cache| {
        let mut bm = cache.borrow_mut();
        bm[(n) as usize].clone()
    }) {
        return ret;
    }
    let ret: i64 = (|| {
        if n == 0 || n == 1 {
            return n;
        }
        fib(n - 1) + fib(n - 2)
    })();
    FIB.with(|cache| {
        let mut bm = cache.borrow_mut();
        bm[(n) as usize] = Some(ret.clone());
    });
    ret
}

// The following code was expanded by `cargo-equip`.

#[allow(clippy::deprecated_cfg_attr)]#[cfg_attr(rustfmt,rustfmt::skip)]#[allow(unused)]pub mod memoise{}
#[allow(clippy::deprecated_cfg_attr)]#[cfg_attr(rustfmt,rustfmt::skip)]#[allow(unused)]pub mod proconio_derive{}
```

</details>

- `rust-analyzer(.exe)`は自動でダウンロードされます。
- `proc-macro`クレートは1.47.0以上のRustでコンパイルされる必要があります。
   現在のツールチェインが1.47.0未満である場合、1.47.0以上のツールチェインを探してそれでコンパイルします。
- `pub use $name::*;`でre-exportedされた手続き型マクロも展開することができます。

## オプション

### `--resolve-cfgs`

1. `#[cfg(恒真)]` (e.g. `cfg(feature = "enabled-feature")`)のアトリビュートを消去します。
2. `#[cfg(恒偽)]` (e.g. `cfg(test)`, `cfg(feature = "disable-feature")`)のアトリビュートが付いたアイテムを消去します。

これは次の割り当てで判定されます。

- [`test`](https://doc.rust-lang.org/reference/conditional-compilation.html#test): `false`
- [`proc_macro`](https://doc.rust-lang.org/reference/conditional-compilation.html#proc_macro): `false`
- `cargo_equip`: `true`
- [`feature`](https://doc.rust-lang.org/cargo/reference/features.html): `bin`側から見て有効化されているもののみ`true`
- それ以外: 不明

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

`#![cfg_attr(cargo_equip, cargo_equip::skip)]`でスキップした場合も有効です。

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
