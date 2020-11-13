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
name = "solve"
version = "0.0.0"
edition = "2018"

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
input                            = { path = "/path/to/input"                                }
output                           = { path = "/path/to/output"                               }
tonelli_shanks                   = { path = "/path/to/tonelli_shanks"                       }
# ...
```

```rust
// verify-helper: PROBLEM https://judge.yosupo.jp/problem/sqrt_mod

// Uncomment this line if you don't use your libraries. (`--check` still works)
//#![cfg_attr(cargo_equip, cargo_equip::skip)]

#[macro_use]
extern crate input as _;

use acl_modint::ModInt;
use std::io::Write as _;
use tonelli_shanks::ModIntBaseExt as _;

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
     Running `/home/ryo/.rustup/toolchains/1.43.0-x86_64-unknown-linux-gnu/bin/cargo check --message-format json -p -p 'serde_derive:1.0.113' -p 'proc-macro2:1.0.10' -p 'serde:1.0.113' -p 'ryu:1.0.5' -p 'syn:1.0.17' -p 'ac-library-rs-parted-internal-math:0.1.0' -p 'num-traits:0.2.14' -p 'typenum:1.12.0' -p 'serde_json:1.0.59' -p 'ac-library-rs-parted-build:0.1.0' -p 'byteorder:1.3.4' -p 'ac-library-rs-parted-modint:0.1.0' -p 'anyhow:1.0.34'`
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running `/home/ryo/.cargo/bin/rustup run nightly cargo udeps --output json -p library-checker --bin sqrt-mod-test`
    Checking library-checker v0.0.0 (/home/ryo/src/github.com/qryxip/oj-verify-playground/verification/library-checker)
    Finished dev [unoptimized + debuginfo] target(s) in 0.17s
info: Loading save analysis from "/home/ryo/src/github.com/qryxip/oj-verify-playground/verification/library-checker/target/debug/deps/save-analysis/sqrt_mod_test-79cb22d236541548.json"
    Bundling the code
    Checking cargo-equip-check-output-54zdzn1al70b1izv v0.1.0 (/tmp/cargo-equip-check-output-54zdzn1al70b1izv)
    Finished dev [unoptimized + debuginfo] target(s) in 0.39s
```

[Submit Info #29645 - Library-Checker](https://judge.yosupo.jp/submission/29645)

## Installation

Install a `nightly` toolchain and [cargo-udeps](https://github.com/est31/cargo-udeps) first.

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

[Releases](https://github.com/qryxip/cargo-equip/releases)

## Usage

Follow these constrants when you writing libraries to bundle.

1. Do not put items with the name names of `#[macro_export]`ed macros in each crate root.

    cargo-equip inserts `pub use crate::{ these_names };` just below each `mod lib_name`.
    Use `#[macro_use]` to import macros in a `bin`.

    ```rust
    // in main source code

    #[macro_use]
    extern crate input as _;
    ```

    `extern crate` items in `bin`s are commented-out.

    ```rust
    // in main source code

    /*#[macro_use]
    extern crate input as _;*/ // `use crate::$name;` is inserted if the rename is not `_`
    ```

2. **To make compatible with Rust 2015**, do not resolve names of crates to bundle directly from [extern prelude](https://doc.rust-lang.org/reference/items/extern-crates.html#extern-prelude).

    Mount them in some module **except the root one** with a `extern crate` item and refer them with relative paths.

    cargo-equip replaces `extern crate` items with `use crate::extern_crate_name_in_main_crate;` except for crates available on AtCoder or CodinGame (e.g. `itertools`).
    [Rename](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml) the libraries not to use directly.

    ```diff
     mod extern_crates {
    -    pub(super) extern crate __another_lib as another_lib;
    +    pub(super) use crate::another_lib;
     }

     use self::extern_crates::another_lib::foo::Foo; // Prepend `self::` to make compatible with Rust 2015
    ```

    If you don't use website where Rust 2018 is unavailable (e.g. AIZU ONLINE JUDGE, yukicoder), you don't have to do this.
    `mod __pseudo_extern_prelude` like this is created in each library as a substitute for extern prelude.
    This `mod __pseudo_extern_prelude` itself is valid in Rust 2015 but unfortunately Rust 2015 cannot resolve the `use another_lib::A;`.

    ```diff
    +mod __pseudo_extern_prelude {
    +    pub(super) use crate::{another_lib1, another_lib2};
    +}
    +use self::__pseudo_extern_prelude::*;
    +
     use another_lib1::A;
     use another_lib2::B;
    ```

3. Use `$crate` instead of `crate` in macros.

    cargo-equip replaces `$crate` in `macro_rules!` with `$crate::extern_crate_name_in_main_crate`.
    `crate` identifiers in `macro_rules!` are not modified.

4. Do not use absolute path as possible.

    cargo-equip replaces `crate` with `crate::extern_crate_name_in_main_crate` and `pub(crate)` with `pub(in crate::extern_crate_name_in_main_crate)`.

    However I cannot ensure this works well.
    Use `self::` and `super::` instead of `crate::`.

    ```diff
    -use crate::foo::Foo;
    +use super::foo::Foo;
    ```

5. Split into small separate crates as possible.

    cargo-equip does not search "dependencies among items".

    On a website other except AtCoder, Split your library into small crates to fit in 64KiB.

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

When you finish preparing your library crates, add them to `[dependencies]` of the `bin`.
If you generate packages automatically with a tool, add them to its template.

If you want to use [rust-lang-ja/ac-library-rs](https://github.com/rust-lang-ja/ac-library-rs), use [qryxip/ac-library-rs-parted](https://github.com/qryxip/ac-library-rs-parted) instead.
ac-library-rs-parted is a collection of 17 crates that process the real ac-library-rs in a `custom-build`.
The `custom-build` is written with `syn 1.0.17` and `proc-macro2 1.0.10` in order not to break [lockfiles for AtCoder](https://github.com/qryxip/cargo-compete/blob/ba8e0e747ed90768d9f50f3061374162dade8450/resources/atcoder-cargo-lock.toml).

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

The constraints for `bin`s are:

1. Do not import macros with `use`. Use them with `#[macro_use]` or with qualified paths.
2. If you create `mod`s, inside them do not resolve names of crates to bundle directly from [extern prelude](https://doc.rust-lang.org/reference/items/extern-crates.html#extern-prelude).

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

Then execute `cargo-equip`.

```console
❯ cargo equip --bin "$name"
```

cargo-equip outputs code like this.
It gives tentative `extern_crate_name`s like `__package_name_0_1_0` to dependencies of the dependencies.

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

cargo-equip does the following modification.

- `bin`
    - If a `#![cfg_attr(cargo_equip, cargo_equip::skip)]` was found, skips the remaining modification, does `cargo check` if `--check` is specified, and outputs the source code as-is.
    - If any, expands `mod $name;`s recursively indenting them except those containing multi-line literals.
    - Replaces some of the `extern crate` items.
    - Prepends a doc comment.
    - Appends the expanded libraries.
- `lib`s
    - Expands `mod $name;` recursively.
    - Replaces some of the `crate` paths.
    - Replaces some of the `extern crate` items.
    - Modifies `macro_rules!`.
    - Inserts `mod __pseudo_extern_prelude { .. }` and `use (self::|$(super::)*)__pseudo_extern_prelude::*;`.
    - Removes `#[cfg(..)]` attributes or their targets if `--resolve-cfg` is specified.
    - Removes doc comments if `--remove docs` is specified.
    - Removes comments if `--remove comments` is specified.
- Whole
    - Minifies the whole output f`--minify all` is specified.
    - Formats the output if `--rustfmt` is specified.

## Options

### `--resolve-cfgs`

1. Removes `#[cfg(always_true_predicate)]` (e.g. `cfg(feature = "enabled-feature")`).
2. Removes items with `#[cfg(always_false_preducate)]` (e.g. `cfg(test)`, `cfg(feature = "disable-feature")`).

Predicates are evaluated according to this rule.

- [`test`](https://doc.rust-lang.org/reference/conditional-compilation.html#test): `false`
- [`proc_macro`](https://doc.rust-lang.org/reference/conditional-compilation.html#proc_macro): `false`
- `cargo_equip`: `true`
- [`feature`](https://doc.rust-lang.org/cargo/reference/features.html): `true` for those enabled
- Otherwise: unknown

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

Removes

- doc comments (`//! ..`, `/// ..`, `/** .. */`, `#[doc = ".."]`) with `--remove docs`.
- comments (`// ..`, `/* .. */`) with `--remove comments`.

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

Minifies

- each expaned library with `--minify lib`.
- the whole code with `--minify all`.

Not that the minification function is incomplete.
Unnecessary spaces may be inserted.

### `--rustfmt`

Formats the output with Rustfmt.

### `--check`

Creates a temporary package that shares the current target directory and execute `cargo check` before outputting.

This flag works even if bundling was skipped by `#![cfg_attr(cargo_equip, cargo_equip::skip)]`.

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

## License

Dual-licensed under [MIT](https://opensource.org/licenses/MIT) or [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0).
