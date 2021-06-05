# cargo-equip

[![CI](https://github.com/qryxip/cargo-equip/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-equip/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-equip/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-equip/branch/master)
[![dependency status](https://deps.rs/repo/github/qryxip/cargo-equip/status.svg)](https://deps.rs/repo/github/qryxip/cargo-equip)
[![Crates.io](https://img.shields.io/crates/v/cargo-equip.svg)](https://crates.io/crates/cargo-equip)
[![Crates.io](https://img.shields.io/crates/l/cargo-equip.svg)](https://crates.io/crates/cargo-equip)

[日本語](https://github.com/qryxip/cargo-equip/blob/master/README-ja.md)

A Cargo subcommand to bundle your code into one `.rs` file for competitive programming.

## Recent updates

See [CHANGELOG.md](https://github.com/qryxip/cargo-equip/blob/master/CHANGELOG.md) or [Releases](https://github.com/qryxip/cargo-equip/releases) for recent updates.

## Example

[Sqrt Mod - Library-Cheker](https://judge.yosupo.jp/problem/sqrt_mod)

```toml
[package]
name = "library-checker"
version = "0.0.0"
edition = "2018"

[dependencies]
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
proconio                          = { version = "0.4.3", features = ["derive"]                          }
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
```

↓

```console
❯ cargo equip \
>       --resolve-cfgs `# Resolve #[cfg(…)]` \
>       --remove docs `# Remove doc comments` \
>       --minify libs `# Minify each library` \
>       --rustfmt `# Apply rustfmt` \
>       --check `# Check the output` \
>       --bin sqrt_mod `# Specify the bin target` | xsel -b
```

[Submit Info #49437 - Library-Checker](https://judge.yosupo.jp/submission/49437)

## Works With

- [x] [fixedbitset 0.4.0](https://docs.rs/crate/fixedbitset/0.4.0)
- [x] [lazy_static 1.4.0](https://docs.rs/crate/lazy_static/1.4.0)
- [x] [maplit 1.0.2](https://docs.rs/crate/maplit/1.0.2)
- [x] [multimap 0.8.3](https://docs.rs/crate/multimap/0.8.3)
- [x] [permutohedron 0.2.4](https://docs.rs/crate/permutohedron/0.2.4)
- [x] [proconio 0.4.3](https://docs.rs/crate/proconio/0.4.3)
- [x] [rustc-hash 1.1.0](https://docs.rs/crate/rustc-hash/1.1.0)
- [x] [smallvec 1.6.1](https://docs.rs/crate/smallvec/1.6.1)
- [x] [strsim 0.10.0](https://docs.rs/crate/strsim/0.10.0)
- [x] [whiteread 0.5.0](https://docs.rs/crate/whiteread/0.5.0)

## Installation

Install a `nightly` toolchain and [cargo-udeps](https://github.com/est31/cargo-udeps) first.

```console
❯ rustup update nightly
```

```console
❯ cargo install cargo-udeps
```

### From Crates.io

```console
❯ cargo install cargo-equip
```

### From `master` branch

```console
❯ cargo install --git https://github.com/qryxip/cargo-equip
```

### GitHub Releases

[Releases](https://github.com/qryxip/cargo-equip/releases)

## Usage

Follow these constrants when you writing libraries to bundle.

1. Set `package.edition` to `"2018"`.

    `"2015"` is not supported.

2. Do not use procedural macros in `lib` crates.

    You can `pub use` them, but cannot call.

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

When you finish preparing your library crates, add them to `[dependencies]` of the `bin`/`example`.
If you generate packages automatically with a tool, add them to its template.

If you want to use [rust-lang-ja/ac-library-rs](https://github.com/rust-lang-ja/ac-library-rs), use [qryxip/ac-library-rs-parted](https://github.com/qryxip/ac-library-rs-parted) instead.

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

The constraints for `bin`s/`example`s are:

1. If you use `proc-macro` crates, make sure the macro names unique.

    If you have trouble about procedural macro names, you can import them with `#[macor_use].`

2. If you create `mod`s, inside them do not resolve names of crates to bundle directly from [extern prelude](https://doc.rust-lang.org/reference/items/extern-crates.html#extern-prelude).

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

Then execute `cargo-equip`.

```console
❯ cargo equip --resolve-cfgs --rustfmt --check --bin "$name"
```

cargo-equip outputs code like this.
It gives tentative `extern_crate_name`s like `__package_name_0_1_0` to dependencies of the dependencies.

```rust
//! # Bundled libraries
//!
//! - `mic 0.0.0 (path+███████████████████████████████████████████)`                                                                                      published in https://github.com/qryxip/mic licensed under `CC0-1.0` as `crate::mic`
//! - `qryxip-competitive-input 0.0.0 (git+https://github.com/qryxip/competitive-programming-library#dadeb6e4685a86f25b4e5c8079f56337321aa12e)`                                                      licensed under `CC0-1.0` as `crate::input`
//! - `qryxip-competitive-partition-point 0.0.0 (git+https://github.com/qryxip/competitive-programming-library#dadeb6e4685a86f25b4e5c8079f56337321aa12e)`                                            licensed under `CC0-1.0` as `crate::partition_point`
//!
//! # Procedural macros
//!
//! - `mic_impl 0.0.0 (path+████████████████████████████████████████████████████)` published in https://github.com/qryxip/mic licensed under `CC0-1.0`
#![allow(unused_imports)]

use input::input;
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
    use crate::mic::__YouCannotRecurseIfTheOutputTypeIsInferred as main;
    let __mic_ans = (move || -> _ {
        input! {a:[u64],}
        a.into_iter()
            .map(|a| (1u64..1_000_000_000).partition_point(|ans| ans.pow(2) < a))
    })();
    let __mic_ans =
        {#[allow(unused_imports)]use/*::*/crate::mic::functions::*;(join("\n"))(__mic_ans)};
    ::std::println!("{}", __mic_ans);
}

// The following code was expanded by `cargo-equip`.

#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def___mic_impl_0_0_0_answer{/* … */}
#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def___mic_impl_0_0_0_solve{/* … */}
#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def_input___input_inner{/* … */}
#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def_input___read{/* … */}
#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def_input_input{/* … */}

#[allow(unused)]
pub mod mic {
    pub mod __macros {}
    // ⋮
}

#[allow(unused)]
pub mod __mic_impl_0_0_0 {
    pub mod __macros {
        pub use crate::{
            __macro_def___mic_impl_0_0_0_answer as answer,
            __macro_def___mic_impl_0_0_0_solve as solve,
        };
    }
    pub use self::__macros::*;
}

#[allow(unused)]
pub mod input {
    pub mod __macros {
        pub use crate::{
            __macro_def_input___input_inner as __input_inner, __macro_def_input___read as __read,
            __macro_def_input_input as input,
        };
    }
    pub use self::__macros::*;
    // ⋮
}

#[allow(unused)]
pub mod partition_point {
    pub mod __macros {}
    // ⋮
}
```

## Expanding procedural macros

cargo-equip can expand procedural macros.

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

↓

<details>
<summary>Output</summary>

```rust
//! # Procedural macros
//!
//! - `memoise 0.3.2 (registry+https://github.com/rust-lang/crates.io-index)`         licensed under `BSD-3-Clause`
//! - `proconio-derive 0.2.1 (registry+https://github.com/rust-lang/crates.io-index)` licensed under `MIT OR Apache-2.0`
#![allow(unused_imports)]

use memoise::memoise;
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
thread_local!(static FIB:std::cell::RefCell<Vec<Option<i64> > > =std::cell::RefCell::new(vec![]));
fn fib_reset() {
    FIB.with(|cache| {
        let mut r = cache.borrow_mut();
        r.clear();
    });
}
fn fib(n: i64) -> i64 {
    if let Some(ret) = FIB.with(|cache| {
        let mut bm = cache.borrow_mut();
        if bm.len() <= (n <= 10) as usize {
            bm.resize((n <= 10) as usize + 1, None);
        }
        bm[(n <= 10) as usize].clone()
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
        bm[(n <= 10) as usize] = Some(ret.clone());
    });
    ret
}

// The following code was expanded by `cargo-equip`.

#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def_memoise_memoise{($(_:tt)*)=>(::std::compile_error!("`memoise` from `memoise 0.3.2` should have been expanded");)}
#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def_memoise_memoise_map{($(_:tt)*)=>(::std::compile_error!("`memoise_map` from `memoise 0.3.2` should have been expanded");)}
#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def_proconio_derive_derive_readable{($(_:tt)*)=>(::std::compile_error!("`derive_readable` from `proconio-derive 0.2.1` should have been expanded");)}
#[cfg_attr(any(),rustfmt::skip)]#[macro_export]macro_rules!__macro_def_proconio_derive_fastout{($(_:tt)*)=>(::std::compile_error!("`fastout` from `proconio-derive 0.2.1` should have been expanded");)}

#[allow(unused)]
pub mod memoise {
    pub mod __macros {
        pub use crate::{
            __macro_def_memoise_memoise as memoise, __macro_def_memoise_memoise_map as memoise_map,
        };
    }
    pub use self::__macros::*;
}

#[allow(unused)]
pub mod proconio_derive {
    pub mod __macros {
        pub use crate::{
            __macro_def_proconio_derive_derive_readable as derive_readable,
            __macro_def_proconio_derive_fastout as fastout,
        };
    }
    pub use self::__macros::*;
}
```

</details>

- `rust-analyzer(.exe)` is automatically downloaded.
- `proc-macro` crates need to be compile with Rust 1.47.0+.
   If version of the active toolchain is less than 1.47.0, cargo-equip finds an alternative toolchain and uses it for compiling `proc-macro`s.
- procedural macros re-exported with `pub use $name::*;` are also able to be expanded.

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
