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
また同一の内容がGitHubのリリースページにあります。

## 機能

- 複数のクレートのバンドル
- cargo-udepsにより使っているライブラリだけバンドル
- 一部のクレートを除外 (`--exclude-{atcoder, codingame}-crates`),
- 手続き型マクロの展開 (`bin`内のみ)
- `#[macro_export]`のスコープを保持
- `#[cfg(..)]`の解決
- コメントおよびdocコメントの削除 (`--remove`)
- minify機能 (`--minify`)
- 生成物をコンパイルが通るかチェック

## 例

[Sqrt Mod - Library-Cheker](https://judge.yosupo.jp/problem/sqrt_mod)

```toml
[package]
name = "library-checker"
version = "0.0.0"
edition = "2018"

[dependencies]
ac-library-rs-parted-modint = { git = "https://github.com/qryxip/ac-library-rs-parted" }
proconio = { version = "0.4.3", features = ["derive"] }
qryxip-competitive-tonelli-shanks = { git = "https://github.com/qryxip/competitive-programming-library" }
# ...
```

```rust
use acl_modint::ModInt;
use proconio::{fastout, input};
use tonelli_shanks::ModIntBaseExt as _;

#[fastout]
fn main() {
    input! {
        yps: [(u32, u32)],
    }

    for (y, p) in yps {
        ModInt::set_modulus(p);
        if let Some(x) = ModInt::new(y).sqrt() {
            println!("{}", x);
        } else {
            println!("-1");
        }
    }
}

mod sub {
    // You can also `use` the crate in submodules.

    #[allow(unused_imports)]
    use proconio::input as _;
}
```

↓

```console
❯ cargo equip \
>       --remove docs `# doc commentを除去` \
>       --minify libs `# ライブラリをそれぞれ一行にminify` \
>       --bin sqrt_mod `# binクレートを指定` | xsel -b
```

[Submit Info #59239 - Library-Checker](https://judge.yosupo.jp/submission/59239)

## 動作するクレート

- [x] [fixedbitset 0.4.0](https://docs.rs/crate/fixedbitset/0.4.0)
- [x] [lazy_static 1.4.0](https://docs.rs/crate/lazy_static/1.4.0)
- [x] [maplit 1.0.2](https://docs.rs/crate/maplit/1.0.2)
- [x] [memoise 0.3.2](https://docs.rs/crate/memoise/0.3.2)
- [x] [multimap 0.8.3](https://docs.rs/crate/multimap/0.8.3)
- [x] [permutohedron 0.2.4](https://docs.rs/crate/permutohedron/0.2.4)
- [x] [proconio 0.4.3](https://docs.rs/crate/proconio/0.4.3)
- [x] [rustc-hash 1.1.0](https://docs.rs/crate/rustc-hash/1.1.0)
- [x] [smallvec 1.6.1](https://docs.rs/crate/smallvec/1.6.1)
- [x] [strsim 0.10.0](https://docs.rs/crate/strsim/0.10.0)
- [x] [whiteread 0.5.0](https://docs.rs/crate/whiteread/0.5.0)

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
❯ cargo install cargo-equip --git https://github.com/qryxip/cargo-equip
```

### GitHub Releases

[バイナリでの提供](https://github.com/qryxip/cargo-equip/releases)もしています。

## 使い方

`cargo-equip`で展開できるライブラリには以下の制約があります。

1. `edition`は`"2018"`にする。

    `"2015"`はサポートしません。

2. `lib`クレートからは手続き型マクロを利用しない。

    `lib`クレートからの手続き型マクロの利用は今のところサポートしていません。`pub use`することは問題ありません。

3. `#[macro_export]`しないマクロの中では`crate`ではなく`$crate`を使う。

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

5. 可能な限り[glob import](https://doc.rust-lang.org/book/ch07-04-bringing-paths-into-scope-with-the-use-keyword.html#the-glob-operator)を使わない。

    cargo-equipは[extern prelude](https://doc.rust-lang.org/reference/names/preludes.html#extern-prelude)や[`#[macro_use]`](https://doc.rust-lang.org/reference/macros-by-example.html#the-macro_use-attribute)を再現するためにglob importを挿入します。
    glob importを使うとこれと衝突する可能性があります。

6. 可能な限りライブラリを小さなクレートに分割する。

    cargo-equipは「クレート内のアイテムの依存関係」を調べることはしません。
    AtCoder以外に参加する場合は、出力結果を制限内(たいてい64KiB程度)に収めるためにできるだけ小さなクレートに分割してください。

    ```console
    .
    ├── a
    │   ├── Cargo.toml
    │   └── src
    │       └── lib.rs
    ├── b
    │   ├── Cargo.toml
    │   └── src
    │       └── lib.rs
    ⋮
    ```

ライブラリが用意できたら、それらを`bin`/`example`側の`Cargo.toml`の`[dependencies]`に加えてください。
コンテスト毎にツールでパッケージを自動生成しているならそれのテンプレートに加えてください。

[rust-lang-ja/ac-library-rs](https://github.com/rust-lang-ja/ac-library-rs)を使いたい場合、[qryxip/ac-library-rs-parted](https://github.com/qryxip/ac-library-rs-parted)を使ってください。
本物のac-library-rsを加工したクレートです。

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
`bin`/`example`側の制約は以下の2つです。

1. 手続き型マクロを利用する場合、マクロ名が被らないように`proc-macro`クレートを選択する。

    Rustのモジュールグラフを解析することは困難極まるため、手続き型マクロについてはプレフィックス抜きのマクロ名のみを頼りに展開します。
    `proc-macro`クレートの跡地にはダミーのアイテムを展開するため、通常のプログラミングのように手続き型マクロを`use`しても問題はありません。

    もし`use`することで問題が起きるなら、`#[macro_use] extern crate crate_name as _;`でインポートすることで、
    これは`use crate::crate_name::__macros::*;`に置き換えられます。

    cargo-equipは[extern prelude](https://doc.rust-lang.org/reference/names/preludes.html#extern-prelude)や[`#[macro_use]`](https://doc.rust-lang.org/reference/macros-by-example.html#the-macro_use-attribute)を再現するためにglob importを挿入します。
    glob importを使うとこれと衝突する可能性があります。

2. 可能な限りglob importを使わない。

    ライブラリ同様にglob importを挿入します。

    ```rust
    pub use __cargo_equip::prelude::*;

    // ︙

    pub mod __cargo_equip {
        pub mod crates {
            // ︙
        }
        // ︙

        pub(crate) prelude {
            pub use crate::__cargo_equip::crates::*;
        }
    }
    ```

```rust
use input::input;
use mic::answer;
use partition_point::RangeBoundsExt as _;

#[answer(join("\n"))]
fn main() -> _ {
    input! {
        a: [u64],
    }
    a.into_iter()
        .map(|a| (1u64..1_000_000_000).partition_point(|ans| ans.pow(2) < a))
}
```

コードが書けたら`cargo equip`で展開します。
`--bin {binの名前}`か`--example {exampleの名前}`、または`--src {binのファイルパス}`で`bin`/`example`を指定してください。
パッケージ内の`bin`/`example`が一つの場合は省略できます。
ただし`default-run`には未対応です。

```console
❯ cargo equip --bin "$name"
```

<!--
コードはこのように展開されます。
`extern_crate_name`が`bin`/`example`側から与えられていないクレートは`__package_name_0_1_0`のような名前が与えられます。

```rust
//! # Bundled libraries
//!
//! - `mic 0.0.0 (path+███████████████████████████████████████████)`                                                                                      published in https://github.com/qryxip/mic licensed under `CC0-1.0` as `crate::__cargo_equip::crates::mic`
//! - `qryxip-competitive-input 0.0.0 (git+https://github.com/qryxip/competitive-programming-library#dadeb6e4685a86f25b4e5c8079f56337321aa12e)`                                                      licensed under `CC0-1.0` as `crate::__cargo_equip::crates::input`
//! - `qryxip-competitive-partition-point 0.0.0 (git+https://github.com/qryxip/competitive-programming-library#dadeb6e4685a86f25b4e5c8079f56337321aa12e)`                                            licensed under `CC0-1.0` as `crate::__cargo_equip::crates::partition_point`
//!
//! # Procedural macros
//!
//! - `mic_impl 0.0.0 (path+████████████████████████████████████████████████████)` published in https://github.com/qryxip/mic licensed under `CC0-1.0`

pub use __cargo_equip::prelude::*;

use input::input;
#[allow(unused_imports)]
use mic::answer;
use partition_point::RangeBoundsExt as _;

/*#[answer(join("\n"))]
fn main() -> _ {
    input! {
        a: [u64],
    }
    a.into_iter()
        .map(|a| (1u64..1_000_000_000).partition_point(|ans| ans.pow(2) < a))
}*/
fn main() {
    #[allow(unused_imports)]
    use crate::__cargo_equip::crates::mic::__YouCannotRecurseIfTheOutputTypeIsInferred as main;
    let __mic_ans = (move || -> _ {
        input! {a:[u64],}
        a.into_iter()
            .map(|a| (1u64..1_000_000_000).partition_point(|ans| ans.pow(2) < a))
    })();
    let __mic_ans = {#[allow(unused_imports)]use/*::*/crate::__cargo_equip::crates::mic::functions::*;join("\n")(__mic_ans)};
    ::std::println!("{}", __mic_ans);
}

// The following code was expanded by `cargo-equip`.

#[allow(unused)]
mod __cargo_equip {
    pub(crate) mod crates {
        pub mod mic {
            use crate::__cargo_equip::preludes::mic::*;
            // ︙
        }

        pub mod __mic_impl_0_0_0 {
            pub use crate::__cargo_equip::macros::__mic_impl_0_0_0::*;
        }

        pub mod input {
            pub use crate::__cargo_equip::macros::input::*;
            // ︙
        }

        pub mod partition_point {
            // ︙
        }
    }

    pub(crate) mod macros {
        pub mod mic {}

        pub mod __mic_impl_0_0_0 {
            pub use crate::{
                __macro_def___mic_impl_0_0_0_answer as answer,
                __macro_def___mic_impl_0_0_0_solve as solve,
            };
        }

        pub mod input {
            pub use crate::{
                __macro_def_input___input_inner as __input_inner,
                __macro_def_input___read as __read, __macro_def_input_input as input,
            };
        }

        pub mod partition_point {}
    }

    pub(crate) mod prelude {
        pub use crate::__cargo_equip::crates::*;
    }

    mod preludes {
        pub mod mic {
            pub(in crate::__cargo_equip) use crate::__cargo_equip::crates::__mic_impl_0_0_0 as mic_impl;
        }

        pub mod __mic_impl_0_0_0 {}

        pub mod input {}

        pub mod partition_point {}
    }
}

#[cfg_attr(any(), rustfmt::skip)]
const _: () = {
    #[macro_export] macro_rules! __macro_def___mic_impl_0_0_0_answer(/* … */);
    #[macro_export] macro_rules! __macro_def___mic_impl_0_0_0_solve(/* … */);
    #[macro_export] macro_rules! __macro_def_input___input_inner(/* … */);
    #[macro_export] macro_rules! __macro_def_input___read(/* … */);
    #[macro_export] macro_rules! __macro_def_input_input(/* … */);
};
```
-->

## `#[cfg(…)]`の解決

cargo-equipはデフォルトで

1. `#[cfg(恒真)]` (e.g. `cfg(feature = "enabled-feature")`)のアトリビュートを消去します。
2. `#[cfg(恒偽)]` (e.g. `cfg(test)`, `cfg(feature = "disable-feature")`)のアトリビュートが付いたアイテムを消去します。

これは次の割り当てで判定されます。

- [`test`](https://doc.rust-lang.org/reference/conditional-compilation.html#test): `false`
- [`proc_macro`](https://doc.rust-lang.org/reference/conditional-compilation.html#proc_macro): `false`
- `cargo_equip`: `true`
- [`feature`](https://doc.rust-lang.org/cargo/reference/features.html): `bin`/`example`側から見て有効化されているもののみ`true`
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

## 出力を`cargo check`

cargo-equipはデフォルトでバンドルしたコードを出力する前にtarget directoryを共有した一時パッケージを作り、それの上で`cargo check`します。

```console
    Checking cargo-equip-check-output-6j2i3j3tgtugeaqm v0.1.0 (/tmp/cargo-equip-check-output-6j2i3j3tgtugeaqm)
    Finished dev [unoptimized + debuginfo] target(s) in 0.11s
```

## 手続き型マクロの展開

cargo-equipは手続き型マクロを展開する機能を持っています。

```rust
use memoise::memoise;
use proconio_derive::fastout;

#[fastout]
fn main() {
    for i in 0..=10 {
        println!("{}", fib(i));
    }
}

#[memoise(n <= 10)]
fn fib(n: i64) -> i64 {
    if n == 0 || n == 1 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}
```

<!--
↓

<details>
<summary>Output</summary>

```rust
//! # Procedural macros
//!
//! - `memoise 0.3.2 (registry+https://github.com/rust-lang/crates.io-index)`         licensed under `BSD-3-Clause`
//! - `proconio-derive 0.2.1 (registry+https://github.com/rust-lang/crates.io-index)` licensed under `MIT OR Apache-2.0`

pub use __cargo_equip::prelude::*;

#[allow(unused_imports)]
use memoise::memoise;
#[allow(unused_imports)]
use proconio_derive::fastout;

/*#[fastout]
fn main() {
    for i in 0..=10 {
        println!("{}", fib(i));
    }
}*/
fn main() {
    let __proconio_stdout = ::std::io::stdout();
    let mut __proconio_stdout = ::std::io::BufWriter::new(__proconio_stdout.lock());
    #[allow(unused_macros)]
    macro_rules!print{($($tt:tt)*)=>{{use std::io::Write as _;::std::write!(__proconio_stdout,$($tt)*).unwrap();}};}
    #[allow(unused_macros)]
    macro_rules!println{($($tt:tt)*)=>{{use std::io::Write as _;::std::writeln!(__proconio_stdout,$($tt)*).unwrap();}};}
    let __proconio_res = {
        for i in 0..=10 {
            println!("{}", fib(i));
        }
    };
    <::std::io::BufWriter<::std::io::StdoutLock> as ::std::io::Write>::flush(
        &mut __proconio_stdout,
    )
    .unwrap();
    return __proconio_res;
}

/*#[memoise(n <= 10)]
fn fib(n: i64) -> i64 {
    if n == 0 || n == 1 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}*/
thread_local!(static FIB:std::cell::RefCell<Vec<Option<i64> > > =std::cell::RefCell::new(vec![None;11usize]));
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

#[allow(unused)]
mod __cargo_equip {
    pub(crate) mod crates {
        pub mod memoise {
            pub use crate::__cargo_equip::macros::memoise::*;
        }

        pub mod proconio_derive {
            pub use crate::__cargo_equip::macros::proconio_derive::*;
        }
    }

    pub(crate) mod macros {
        pub mod memoise {
            pub use crate::{
                __macro_def_memoise_memoise as memoise,
                __macro_def_memoise_memoise_map as memoise_map,
            };
        }

        pub mod proconio_derive {
            pub use crate::{
                __macro_def_proconio_derive_derive_readable as derive_readable,
                __macro_def_proconio_derive_fastout as fastout,
            };
        }
    }

    pub(crate) mod prelude {
        pub use crate::__cargo_equip::crates::*;
    }

    mod preludes {
        pub mod memoise {}

        pub mod proconio_derive {}
    }
}

#[cfg_attr(any(), rustfmt::skip)]
const _: () = {
    #[macro_export] macro_rules! __macro_def_memoise_memoise(($(_:tt)*)=>(::std::compile_error!("`memoise` from `memoise 0.3.2` should have been expanded");));
    #[macro_export] macro_rules! __macro_def_memoise_memoise_map(($(_:tt)*)=>(::std::compile_error!("`memoise_map` from `memoise 0.3.2` should have been expanded");));
    #[macro_export] macro_rules! __macro_def_proconio_derive_derive_readable(($(_:tt)*)=>(::std::compile_error!("`derive_readable` from `proconio-derive 0.2.1` should have been expanded");));
    #[macro_export] macro_rules! __macro_def_proconio_derive_fastout(($(_:tt)*)=>(::std::compile_error!("`fastout` from `proconio-derive 0.2.1` should have been expanded");));
};
```

</details>
-->

- `proc-macro`クレートは1.48.0以上のRustでコンパイルされる必要があります。
   現在のツールチェインが1.48.0未満である場合、1.48.0以上のツールチェインを探してそれでコンパイルします。
- `pub use $name::*;`でre-exportedされた手続き型マクロも展開することができます。

## オプション

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

### `--no-resolve-cfgs`

`#[cfg(…)]`を解決しません。

### `--no-rustfmt`

出力をRustfmtでフォーマットするのをスキップします。

### `--no-check`

出力を`cargo check`にかけるのをスキップします。

## ライセンス

[MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)のデュアルライセンスです。
