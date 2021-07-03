# Changelog

## [0.17.0] - 2021-07-03Z

### Changed

- Improved the help. ([#143](https://github.com/qryxip/cargo-equip/pull/143))

    Now you can read the detailed help with `--help` while `cargo equip -h` still prints a short and concise overview.

- Changed the format. ([#144](https://github.com/qryxip/cargo-equip/pull/144))

    - Moved `__bundled` to `__cargo_equip::crates`.
    - Moved `__bundled::*::__macros` to `__cargo_equip::macros`.
    - Now creates `__cargo_equip::prelude` and glob-uses it instead of declaring `__prelude_for_main_crate!`.
    - Unified `__bundled::*::{__external_local_inner_macros, __pseudo_extern_prelude}` into `__cargo_equip::preludes`.

    ```rust
    //! # Bundled libraries
    //! ︙

    pub use __cargo_equip::prelude::*;

    // ︙

    // The following code was expanded by `cargo-equip`.

    mod __cargo_equip {
        pub(crate) mod crates {/* … */}
        pub(crate) mod macros {/* … */}
        pub(crate) mod prelude {/* … */}
        mod preludes {/* … */}
    }

    #[cfg_attr(any(), rustfmt::skip)]
    const _: () = {
        // ︙
    };
    ```

- Updated rust-analyzer to `2021-06-28`. ([#145](https://github.com/qryxip/cargo-equip/pull/145))

## [0.16.0] - 2021-06-26Z

### Changed

- `--resolve-cfgs` `--rustfmt` `--check` by default. ([#139](https://github.com/qryxip/cargo-equip/pull/139))

    Instead, added `--no-resolve-cfgs`, `--no-rustfmt`, and `--no-check`.

    ```console
    ❯ cargo-equip --resolve-cfgs --rustfmt --check
    warning: `--resolve-cfgs` is deprecated. `#[cfg(..)]`s are resolved by default
    warning: `--rustfmt` is deprecated. the output is formatted by default
    warning: `--check` is deprecated. the output is checked by default
    ```

- Removed the warning message for `pub(crate)` in libraries. ([#138](https://github.com/qryxip/cargo-equip/pull/138))

- Updated `--exclude-{atcoder, codingame}-crates`. ([#140](https://github.com/qryxip/cargo-equip/pull/140))

    Added `…#proconio:0.3.8` to `--exclude-atcoder-crates`.

- Updated rust-analyzer to `2021-06-21`. ([#141](https://github.com/qryxip/cargo-equip/pull/141))

## [0.15.0] - 2021-06-11Z

### Changed

- cargo-equip now inserts `#[allow(unused_imports)]` for only **seemingly** procedural macros.

    ```rust
    use proconio::{fastout, input};
    ```

    ↓

    ```rust
    use proconio::{/*fastout,*/ input};
    #[allow(unused_imports)]
    use proconio::fastout;
    ```

## [0.14.0] - 2021-06-06Z

### Added

- Scopes for macros are now preserved. ([#126](https://github.com/qryxip/cargo-equip/pull/126))

    Now you can `use` macros anywhere.

    ```rust
    use proconio::input;

    fn main() {
        input! {
            n: usize,
        }
    }
    ```

- Dummy macros are now expanded for `proc-macro` crates. ([#126](https://github.com/qryxip/cargo-equip/pull/126))

    Now you can also `use` procedural macros.

    ```rust
    use proconio::fastout;

    #[fastout]
    fn main() {
        println!("Hi");
    }
    ```

- Now `#[[warn/deny/forbid](missing-(crate-level-)docs)]` attributes are removed when `--remove docs` is enabled. ([#126](https://github.com/qryxip/cargo-equip/pull/126))

    Now you can remove doc comments from [smallvec](https://docs.rs/crate/smallvec).

- Modules in main crates are now supported. ([#129](https://github.com/qryxip/cargo-equip/pull/129))

    ```rust
    mod sub {
        use smallvec::{smallvec, SmallVec};

        pub(crate) fn f() {
            let _: SmallVec<[(); 0]> = smallvec![];
        }
    }
    ```

### Changed

- Dropped the support for Rust 2015. ([#126](https://github.com/qryxip/cargo-equip/pull/126))

- `#[allow(unused_imports)]` will be always inserted. ([#126](https://github.com/qryxip/cargo-equip/pull/126))

- Updated rust-analyzer to `2021-05-31`. ([#130](https://github.com/qryxip/cargo-equip/pull/130))

### Fixed

- Fixed order of `#[macro_exported]` macros. ([#126](https://github.com/qryxip/cargo-equip/pull/126))

- Fixed [a problem where `attr`s for attribute macros contain parentheses](https://github.com/qryxip/cargo-equip/issues/128). ([#130](https://github.com/qryxip/cargo-equip/pull/130))

    ```rust
    #[memoise(n <= 10)]
             ^       ^
    ```

## [0.13.1] - 2021-05-22Z

### Fixed

- Fixed [the problem where `cargo +nightly equip` does not work](https://github.com/qryxip/cargo-equip/issues/118).

- Enabled minifying `1. ..x` correctly.

## [0.13.0] - 2021-05-15Z

### Changed

- Now cargo-equip replaces paths and `use` trees that have leading semicolons. ([#113](https://github.com/qryxip/cargo-equip/pull/113))

    ```rust
    use ::{lib1::A, lib2::B}
    ```

    ↓

    ```rust
    use /*::*/crate::{/*lib1*/__lib1_0_1_0::A, /*lib2*/lib2_0_1_0::B}
    ```

- Updated rust-analyzer to `2021-05-10`. ([#114](https://github.com/qryxip/cargo-equip/pull/114))

## [0.12.2] - 2021-05-11Z

### Changed

- Removed `#[allow(clippy::deprecated_cfg_attr)]`. ([#110](https://github.com/qryxip/cargo-equip/pull/110))

    ```diff
    -#[allow(clippy::deprecated_cfg_attr)]#[cfg_attr(rustfmt,rustfmt::skip)]#[allow(unused)]pub mod lib{/* ... */}
    +                                     #[cfg_attr(any()  ,rustfmt::skip)]#[allow(unused)]pub mod lib{/* ... */}
    ```

- Tokens from procedural macros will be minified. ([#111](https://github.com/qryxip/cargo-equip/pull/111))

## [0.12.1] - 2021-05-02Z

### Fixed

- Recognizes `#[macro_export(local_inner_macros)]`. ([#105](https://github.com/qryxip/cargo-equip/pull/105))

- Fixed a problem where `$crate`s are not replaced with `$crate::lib_name` in a certain case. ([#106](https://github.com/qryxip/cargo-equip/pull/105))

- Fixed a problem where `--remove` option could not be used for CRLF code. ([#106](https://github.com/qryxip/cargo-equip/pull/105))

    With the above 3 fixtures, you can bundle [proconio](https://docs.rs/crate/proconio).

    ```console
    ❯ cat <<EOF >./src/main.rs
    heredoc> #[macro_use]
    heredoc> extern crate proconio as _;
    heredoc>
    heredoc> #[fastout]
    heredoc> fn main() {
    heredoc>     input!(abs: [(u64, u64)]);
    heredoc>     for (a, b) in abs {
    heredoc>         println!("{}", a + b);
    heredoc>     }
    heredoc> }
    heredoc> EOF
    ❯ cargo equip \
    >       --resolve-cfgs \
    >       --remove docs \
    >       --minify libs \
    >       --rustfmt \
    >       --check \
    >       -o ./bundled.rs
    ❯ ./run-cargo-equip.bash -o ./bundled.rs
         Running `/home/ryo/.cargo/bin/rustup run nightly cargo udeps --output json -p bundle-proconio --bin bundle-proconio`
        Checking bundle-proconio v0.1.0 (/home/ryo/src/local/bundle-proconio)
        Finished dev [unoptimized + debuginfo] target(s) in 0.45s
    info: Loading save analysis from "/home/ryo/src/local/bundle-proconio/target/debug/deps/save-analysis/bundle_proconio-31a013a4acd96cad.json"
         Running `/home/ryo/.cargo/bin/rustup run stable-x86_64-unknown-linux-gnu cargo check --message-format json -p 'bundle-proconio:0.1.0' --bin bundle-proconio`
        Checking bundle-proconio v0.1.0 (/home/ryo/src/local/bundle-proconio)
        Finished dev [unoptimized + debuginfo] target(s) in 0.44s
        Spawning `/home/ryo/.cache/cargo-equip/rust-analyzer-2021-03-29 proc-macro`
         Readied `#[derive_readable]`
         Readied `#[fastout]`
        Bundling the code
    warning: found `crate` paths. replacing them with `crate::proconio`
         Reading the license file of `lazy_static 1.4.0 (registry+https://github.com/rust-lang/crates.io-index)`
         Reading the license file of `proconio 0.4.3 (registry+https://github.com/rust-lang/crates.io-index)`
        Checking cargo-equip-check-output-fs0en4z4r1d3gd3e v0.1.0 (/tmp/cargo-equip-check-output-fs0en4z4r1d3gd3e)
        Finished dev [unoptimized + debuginfo] target(s) in 0.19s
    ❯ stat -c %s ./bundled.rs
    18024
    ```

    [Submission Info #46571 - Library Checker](https://judge.yosupo.jp/submission/46571)

## [0.12.0] - 2021-05-01Z

### Added

- Supports `#[cfg(..)] extern crate ..;` items.

    Now you can bundle [lazy\_static](https://docs.rs/crate/lazy_static/1.4.0).

    ```console
    ❯ cat >./src/main.rs <<EOF
    heredoc> #[macro_use]
    heredoc> extern crate lazy_static as _;
    heredoc>
    heredoc> fn main() {
    heredoc>     println!("{}", *N);
    heredoc> }
    heredoc>
    heredoc> lazy_static! {
    heredoc>     static ref N: u32 = 42;
    heredoc> }
    heredoc> EOF
    ❯ cargo equip \
    >       --resolve-cfgs \
    >       --check \
    >       --remove docs \
    >       --minify libs \
    >       --rustfmt \
    >       -o ./bundled.rs
         Running `/home/ryo/.cargo/bin/rustup run nightly cargo udeps --output json -p bundle-proconio --bin bundle-proconio`
        Checking bundle-proconio v0.1.0 (/home/ryo/src/local/bundle-proconio)
        Finished dev [unoptimized + debuginfo] target(s) in 0.43s
    info: Loading save analysis from "/home/ryo/src/local/bundle-proconio/target/debug/deps/save-analysis/bundle_proconio-65ed1cd2cb2d0758.json"
        Bundling the code
         Reading the license file of `lazy_static 1.4.0 (registry+https://github.com/rust-lang/crates.io-index)`
        Checking cargo-equip-check-output-7l11rus91rnoie88 v0.1.0 (/tmp/cargo-equip-check-output-7l11rus91rnoie88)
        Finished dev [unoptimized + debuginfo] target(s) in 0.45s
    ❯ stat -c %s ./bundled.rs
    4353
    ❯ rustc --edition 2018 -o /tmp/bundled ./bundled.rs && /tmp/bundled
    42
    ```

- Clones Git repositories if license files are not found in `manifest_dir`s.

### Changed

- cargo-equip won't error for unresolved `extern crate` items.

## [0.11.1] - 2021-03-30Z

### Added

- Enabled using `aarch64` rust-analyzers. ([#99](https://github.com/qryxip/cargo-equip/pull/99))

- Enabled running for `example` targets. ([#100](https://github.com/qryxip/cargo-equip/pull/100))

    ```console
    ❯ cargo equip -h | head -n 9 | tail -5
    USAGE:
        cargo equip [OPTIONS]
        cargo equip [OPTIONS] --src <PATH>
        cargo equip [OPTIONS] --bin <NAME>
        cargo equip [OPTIONS] --example <NAME>
    ```

    ```console
    ❯ cargo equip … --example atcoder-abc188-a
    ```

### Changed

- Updated rust-analzyer to `2021-03-29`. ([#99](https://github.com/qryxip/cargo-equip/pull/99))

### Fixed

- `lib`/`proc-macro` crates in the same packages will be included. ([#100](https://github.com/qryxip/cargo-equip/pull/100))

## [0.11.0] - 2021-03-27Z

### Changed

- Changed the process order for `bin` crates. ([#92](https://github.com/qryxip/cargo-equip/pull/92))

    `extern crate` items are replaced after procedural macros are expanded.

    ```rust
    let foo = /*foo!()*/{
        /*extern crate foo as __foo ;*/
        use crate::foo as __foo;
        __foo::Foo::new()
    };
    ```

- Added `https://github.com/rust-lang/crates.io-index#proconio:0.3.7` to `--exclude-atcoder-crates`. ([#97](https://github.com/qryxip/cargo-equip/pull/97))

    See [statiolake/proconio-rs#18](https://github.com/statiolake/proconio-rs/pull/18).

## [0.10.0] - 2021-02-28Z

### Changed

- cargo-equip now expand procedural macros using `rust-analyzer(.exe)`.

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

    - `watt` crate no longer necessary.
    - `rust-analyzer(.exe)` is automatically downloaded.
    - `proc-macro` crates need to be compile with Rust 1.47.0+.
       If version of the active toolchain is less than 1.47.0, cargo-equip finds an alternative toolchain and uses it for compiling `proc-macro`s.
    - procedural macros re-exported with `pub use $name::*;` are also able to be expanded.

### Fixed

- Enabled handling non-[`Meta`](https://docs.rs/syn/1/syn/enum.Meta.html) attribute macros such as `#[memoise(n <= 100)]`.

- Fixed a problem where `extern crate` items in a `bin` crate are not removed properly.

## [0.9.3] - 2021-02-13Z

### Fixed

- Fixed inability to `cargo install cargo-equip` without `--locked`. ([#79](https://github.com/qryxip/cargo-equip/pull/79))

## [0.9.2] - 2020-12-23Z

### Added

- Enabled expanding procedural macros. ([#76](https://github.com/qryxip/cargo-equip/pull/76))

    The `proc_macro` crates need to use [watt](https://github.com/dtolnay/watt) and have `package.metadata` like this.

    ```toml
    [package.metadata.cargo-equip.watt.proc-macro]
    input = "$OUT_DIR/macros_impl.wasm"

    [package.metadata.cargo-equip.watt.proc-macro-attribute]
    fastout = "$OUT_DIR/macros_impl.wasm"
    ```

    [Example](https://github.com/qryxip/competitive-programming-library/tree/master/proc-macros/fastout)

    ```toml
    [dependencies]
    qryxip-competitive-fastout = { git = "https://github.com/qryxip/competitive-programming-library" }
    ```

    ```rust
    #[macro_use]
    extern crate fastout as _;

    #[fastout]
    fn main() {
        for i in 0..10 {
            println!("{}", i);
        }
    }
    ```

### Changed

- Changed the format of `"# Bundled libraries"`. ([#77](https://github.com/qryxip/cargo-equip/pull/77))

    ```toml
    //! # Bundled libraries
    //!
    //! - `ac-library-rs-parted-internal-math 0.1.0 (git+https://github.com/qryxip/ac-library-rs-parted#0b35cf2fd575eaf8ace84340722d6cefb7e82d14)`                                    licensed under `CC0-1.0` as `crate::__ac_library_rs_parted_internal_math_0_1_0`
    //! - `ac-library-rs-parted-modint 0.1.0 (git+https://github.com/qryxip/ac-library-rs-parted#0b35cf2fd575eaf8ace84340722d6cefb7e82d14)`                                           licensed under `CC0-1.0` as `crate::acl_modint`
    //! - `local 0.0.0 (path+██████████████████████████████████████████████████)`                                                                            published in **missing** licensed under `CC0-1.0` as `crate::local`
    //! - `qryxip-competitive-input 0.0.0 (git+https://github.com/qryxip/competitive-programming-library#0d98777f6af8d6174419bf9b9a202c7f023d6857)`                                   licensed under `CC0-1.0` as `crate::input`
    //! - `qryxip-competitive-tonelli-shanks 0.0.0 (git+https://github.com/qryxip/competitive-programming-library#0d98777f6af8d6174419bf9b9a202c7f023d6857)`                          licensed under `CC0-1.0` as `crate::tonelli_shanks`
    //! - `qryxip-competitive-xorshift 0.0.0 (git+https://github.com/qryxip/competitive-programming-library#0d98777f6af8d6174419bf9b9a202c7f023d6857)`                                licensed under `CC0-1.0` as `crate::__qryxip_competitive_xorshift_0_0_0`
    //!
    //! # Procedural macros
    //!
    //! - `qryxip-competitive-fastout 0.0.0 (git+https://github.com/qryxip/competitive-programming-library#0d98777f6af8d6174419bf9b9a202c7f023d6857)` licensed under `CC0-1.0`
    ```

## [0.9.1] - 2020-12-07Z

### Changed

- Changed the format of `"# Bundled libraries"`.  ([#72](https://github.com/qryxip/cargo-equip/pull/72))

    ```rust
    //! - `ac-library-rs-parted-internal-math 0.1.0 (git+https://github.com/qryxip/ac-library-rs-parted#3fc14c009609d8f0a3db8332493dafe457c3460f)`                      as `crate::__ac_library_rs_parted_internal_math_0_1_0`               (license: `CC0-1.0`, repository: https://github.com/qryxip/ac-library-rs-parted)
    //! - `ac-library-rs-parted-modint 0.1.0 (git+https://github.com/qryxip/ac-library-rs-parted#3fc14c009609d8f0a3db8332493dafe457c3460f)`                             as `crate::acl_modint`                                               (license: `CC0-1.0`, repository: https://github.com/qryxip/ac-library-rs-parted)
    //! - `maplit 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)`                                                                                        as `crate::maplit`                                                   (license: `MIT/Apache-2.0`, repository: https://github.com/bluss/maplit)
    //! - `qryxips-competitive-programming-library-buffered-print 0.0.0 (path+█████████████████████████████████████████████████████████████████████████████████████)`   as `crate::buffered_print`                                           (license: `CC0-1.0`, repository: https://github.com/qryxip/oj-verify-playground)
    //! - `qryxips-competitive-programming-library-input 0.0.0 (path+████████████████████████████████████████████████████████████████████████████)`                     as `crate::input`                                                    (license: `CC0-1.0`, repository: https://github.com/qryxip/oj-verify-playground)
    //! - `qryxips-competitive-programming-library-tonelli-shanks 0.0.0 (path+███████████████████████████████████████████████████████████████████████████████████████)` as `crate::tonelli_shanks`                                           (license: `CC0-1.0`, repository: https://github.com/qryxip/oj-verify-playground)
    //! - `qryxips-competitive-programming-library-xorshift 0.0.0 (path+████████████████████████████████████████████████████████████████████████████)`                  as `crate::__qryxips_competitive_programming_library_xorshift_0_0_0` (license: `CC0-1.0`, repository: https://github.com/qryxip/oj-verify-playground)
    ```

## [0.9.0] - 2020-11-27Z

### Added

- Added `--exclude-atcoder-crates`, `--exclude-codingame-crates`, and `--exclude <SPEC>...` options. ([#68](https://github.com/qryxip/cargo-equip/pull/68))

    ```console
            --exclude <SPEC>...           Exclude library crates from bundling
            --exclude-atcoder-crates      Alias for `--exclude https://github.com/rust-lang/crates.io-index#alga:0.9.3 ..`
            --exclude-codingame-crates    Alias for `--exclude https://github.com/rust-lang/crates.io-index#chrono:0.4.9 ..`
    ```

### Changed

- Changed format of the "# Bundled libraries". ([#64](https://github.com/qryxip/cargo-equip/pull/64))

    ```rust
    //! # Bundled libraries
    //!
    //! - `ac-library-rs-parted-internal-math v0.1.0` → `crate::__ac_library_rs_parted_internal_math_0_1_0` (source: `git+https://github.com/qryxip/ac-library-rs-parted#3fc14c009609d8f0a3db8332493dafe457c3460f`, license: `CC0-1.0`)
    //! - `ac-library-rs-parted-modint v0.1.0` → `crate::acl_modint` (source: `git+https://github.com/qryxip/ac-library-rs-parted#3fc14c009609d8f0a3db8332493dafe457c3460f`, license: `CC0-1.0`)
    //! - `input v0.0.0` → `crate::input` (source: `git+https://github.com/qryxip/oj-verify-playground#63ddefa84d96b16cdb7f85e70dcdc4f283f57391`, license: `CC0-1.0`)
    //! - `output v0.0.0` → `crate::output` (source: `git+https://github.com/qryxip/oj-verify-playground#63ddefa84d96b16cdb7f85e70dcdc4f283f57391`, license: `CC0-1.0`)
    //! - `tonelli_shanks v0.0.0` → `crate::tonelli_shanks` (source: `git+https://github.com/qryxip/oj-verify-playground#63ddefa84d96b16cdb7f85e70dcdc4f283f57391`, license: `CC0-1.0`)
    //! - `xorshift v0.0.0` → `crate::__xorshift_0_0_0` (source: `git+https://github.com/qryxip/oj-verify-playground#63ddefa84d96b16cdb7f85e70dcdc4f283f57391`, license: `CC0-1.0`)
    ```

- Now cargo-equip adds "License and copyright notices" to the output when you are using other people's libraries. ([#64](https://github.com/qryxip/cargo-equip/pull/64),  [#68](https://github.com/qryxip/cargo-equip/pull/68))

    ```rust
    //! # License and Copyright Notices
    //!
    //! - `maplit 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)`
    //!
    //!     ```text
    //!     Copyright (c) 2015
    //!
    //!     Permission is hereby granted, free of charge, to any
    //!     person obtaining a copy of this software and associated
    //!     documentation files (the "Software"), to deal in the
    //!     Software without restriction, including without
    //!     limitation the rights to use, copy, modify, merge,
    //!     publish, distribute, sublicense, and/or sell copies of
    //!     the Software, and to permit persons to whom the Software
    //!     is furnished to do so, subject to the following
    //!     conditions:
    //!
    //!     The above copyright notice and this permission notice
    //!     shall be included in all copies or substantial portions
    //!     of the Software.
    //!
    //!     THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
    //!     ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
    //!     TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
    //!     PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
    //!     SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
    //!     CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
    //!     OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
    //!     IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    //!     DEALINGS IN THE SOFTWARE.
    //!     ```
    ```

    Currently, only `CC0-1.0`, `Unlicense`, `MIT` and `Apache-2.0` are supported.

- Now cargo-equip considers [platform specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies). ([#66](https://github.com/qryxip/cargo-equip/pull/66))

- The 39 crate available on AtCoder are no longer automatically excluded from bundling. ([#68](https://github.com/qryxip/cargo-equip/pull/68))

    Enable `--exlude-atcoder-crates` to exclude them.

    ```console
    note: attempted to bundle with the following crate(s), which are available on AtCoder. to exclude them from bundling, run with `--exclude-atcoder-crates`

    - `im-rc 14.3.0 (registry+https://github.com/rust-lang/crates.io-index)`
    - `rand_core 0.5.1 (registry+https://github.com/rust-lang/crates.io-index)`
    ```

### Fixed

- `extern crate {core, alloc, std};` will be just ignored. ([#67](https://github.com/qryxip/cargo-equip/pull/67))

- Replaced `-p <libraries>...` for `cargo check` with `--bin <binary>` when obtaining `$OUT_DIR`s. ([#69](https://github.com/qryxip/cargo-equip/pull/69))

    Previously, `cargo check` sometimes failed.

## [0.8.0] - 2020-11-13Z

### Changed

- cargo-equip now creates "pseudo extern prelude" in each library. ([#60](https://github.com/qryxip/cargo-equip/pull/60))

    **Unless you use AIZU ONLINE JUDGE or yukicoder**, you no longer need to declare `extern crate` in libraries.

    ```diff
    +mod __pseudo_extern_prelude {
    +    pub(super) use crate::{another_lib1, another_lib2};
    +}
    +use self::__pseudo_extern_prelude::*;
    +
     use another_lib1::A;
     use another_lib2::B;
    ```

- Declaring `extern crate .. as ..` in a root module will produce a warning. ([#58](https://github.com/qryxip/cargo-equip/pull/58))

    To make your libraries compatible with Rust 2015, create a sub module and declare in it.

    ```rust
    mod extern_crates {
        pub(super) extern crate __another_lib as another_lib;
    }

    use self::extern_crates::another_lib::foo::Foo;
    ```

- Changed `#[allow(dead_code)]` to `#[allow(unused)]`. ([#60](https://github.com/qryxip/cargo-equip/pull/60))

    [`unused`](https://doc.rust-lang.org/rustc/lints/groups.html) is a group of `dead-code`, `unused-imports`, and so on.

    ```diff
    -#[allow(dead_code)]
    +#[allow(unused)]
     mod my_lib {
         // ...
     }
    ```

### Fixed

- Now libraries are expanded as `pub mod`. ([#58](https://github.com/qryxip/cargo-equip/pull/58))

    ```diff
     #[allow(unused)]
    -mod my_lib {
    +pub mod my_lib {
         // ...
     }
    ```

## [0.7.2] - 2020-11-07Z

### Added

- Now you can skip the processes except `--check`. ([#48](https://github.com/qryxip/cargo-equip/pull/48))

    ```rust
    #![cfg_attr(cargo_equip, cargo_equip::skip)]
    ```

- Enabled expanding code generated in `custom-build`. ([#49](https://github.com/qryxip/cargo-equip/pull/49))

    ```rust
    ::core::include!(::core::concat!(::core::env!("OUT_DIR", "/generated.rs")));
    ```

    Now you can use [qryxip/ac-library-rs-parted](https://github.com/qryxip/ac-library-rs-parted) with cargo-equip.

### Fixed

- Fixed the problem where cargo-equip did not treat `proc-macro` crate as libraries. ([#50](https://github.com/qryxip/cargo-equip/pull/50))

## [0.7.1] - 2020-11-07Z

### Added

- Now it gives pseudo `extern_crate_name`s like `"__internal_lib_0_1_0"` to dependencies of dependencies. ([#43](https://github.com/qryxip/cargo-equip/pull/43))

    You no longer need to include all of the libraries in `[dependencies]`.

- Supports `extern crate $name as $rename` in `bin`s. ([#41](https://github.com/qryxip/cargo-equip/pull/41))

    ```rust
    extern crate foo as foo_;
    ```

### Fixed

- Now it correctly processes outputs of cargo-udeps. ([#43](https://github.com/qryxip/cargo-equip/pull/43))

    The names are [`name_in_toml`](https://docs.rs/cargo/0.48.0/cargo/core/dependency/struct.Dependency.html#method.name_in_toml)s. Previously they were treated as [`extern_crate_name`](https://docs.rs/cargo/0.48.0/cargo/core/struct.Resolve.html#method.extern_crate_name)s.

## [0.7.0] - 2020-11-03Z

### Changed

- cargo-equip no longer consider modules. ([#41](https://github.com/qryxip/cargo-equip/pull/41))

    Split your library into separate small crates.

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

- Stopped erasing non `mod` items just below each `lib` crates. ([#41](https://github.com/qryxip/cargo-equip/pull/41))

- Now cargo-equip inserts `pub use crate::{ exported_macros }` just below each `mod lib_name`.

- Stopped excluding `$ crate :: ident !` parts in `macro_rules!`. ([#41](https://github.com/qryxip/cargo-equip/pull/41))

- Now processes `#[macro_use] extern crate $name as _;` in `bin`s. ([#41](https://github.com/qryxip/cargo-equip/pull/41))

    ```rust
    // in main source code
    #[macro_use]
    extern crate input as _;
    ```

    ↓

    ```rust
    // in main source code
    /*#[macro_use]
    extern crate input as _;*/
    ```

- `#![cfg_attr(cargo_equip, cargo_equip::equip)]` no longer requried. ([#41](https://github.com/qryxip/cargo-equip/pull/41))

    ```diff
    -#![cfg_attr(cargo_equip, cargo_equip::equip)]
    ```

- `#![cfg_attr(cargo_equip, cargo_equip::use_another_lib)]` no longer requried. ([#41](https://github.com/qryxip/cargo-equip/pull/41))

    ```diff
    -#[cfg_attr(cargo_equip, cargo_equip::use_another_lib)]
     extern crate __another_lib as another_lib;
    ```

- `#![cfg_attr(cargo_equip, cargo_equip::translate_dolalr_crates)]` no longer requried. ([#41](https://github.com/qryxip/cargo-equip/pull/41))

    ```diff
    -#[cfg_attr(cargo_equip, cargo_equip::translate_dollar_crates)]
     #[macro_export]
     macro_rules! foo { .. }
    ```

## [0.6.0] - 2020-10-24Z

### Added

- Added `--resolve-cfgs` option. ([#35](https://github.com/qryxip/cargo-equip/pull/35))

    This option removes:

    1. `#[cfg(always_true_condition)]` (e.g. `cfg(feature = "enabled-feature")`)
    2. Items with `#[cfg(always_false_condition)]` (e.g. `cfg(test)`, `cfg(feature = "disable-feature")`)

### Removed

- Removed `--remove test-items` option. ([#35](https://github.com/qryxip/cargo-equip/pull/35))

    Use `--resolve-cfgs` instead.

## [0.5.3] - 2020-10-24Z

### Fixed

- Fixed the minification function. ([#33](https://github.com/qryxip/cargo-equip/pull/33))

    Previously, `x < -y` was converted into `x<-y`.

## [0.5.2] - 2020-10-22Z

### Added

- Improved the minification function. ([#31](https://github.com/qryxip/cargo-equip/pull/31))

## [0.5.1] - 2020-10-17Z

### Added

- Now cargo-equip replaces `crate` path prefixes in library code with `crate::extern_crate_name_in_main_crate`. ([#29](https://github.com/qryxip/cargo-equip/pull/29))

## [0.5.0] - 2020-10-03Z

### Changed

- Changed the process of bundling. ([#27](https://github.com/qryxip/cargo-equip/pull/27))

    Now cargo-equip "expands" each `mod`s in `lib.rs`, remove unused ones, then minify the code by `lib`.

- `#[cfg_attr(cargo_equip, cargo_equip::equip)]` now targets root modules instead of `use` statements. ([#27](https://github.com/qryxip/cargo-equip/pull/27))

    All of the `use` statements in the crate root that start with `::` are expanded.

    ```rust
    #![cfg_attr(cargo_equip, cargo_equip::equip)]

    use ::__foo::{a::A, b::B};
    use ::__bar::{c::C, d::D};
    ```

- Renamed `--minify mods` to `--minify libs`. ([#27](https://github.com/qryxip/cargo-equip/pull/27))

## [0.4.1] - 2020-09-28Z

### Added

- Enabled writing `package.metadata.cargo-equip.module-dependencies` also in libraries. ([#25](https://github.com/qryxip/cargo-equip/pull/25))

    They are merged into one graph.

    ```toml
    [package.metadata.cargo-equip.module-dependencies]
    "crate::a" = []
    "crate::b" = ["crate::b"]
    "crate::c" = ["::__another_lib::d"]
    "::__another_lib::d" = ["::__another_lib::e"]
    ```

## [0.4.0] - 2020-09-27Z

### Added

- Now cargo-equip supports libraries that depend on each other. ([#23](https://github.com/qryxip/cargo-equip/pull/23))

    Statements like this will be converted to `use crate::..;`.

    ```
    #[cfg_attr(cargo_equip, cargo_equip::use_another_lib)]
    extern crate __another_lib as another_lib;
    ```

### Changed

- Each library will be expanded in `mod`. ([#23](https://github.com/qryxip/cargo-equip/pull/23))

    More constraints were introduced. See [README-ja](https://github.com/qryxip/cargo-equip/blob/master/README-ja.md) for more details.

    ```rust
    #[allow(dead_code)]
    pub mod __lib {
        pub mod a {}
        pub mod b {}
        pub mod c {}
    }
    ```

- cargo-equip will use `[package.metadata.cargo-equip.module-dependencies]` in the main crate. ([#23](https://github.com/qryxip/cargo-equip/pull/23))

    Package metadata in libraries is no longer read.

    And the format was also changed.

    ```toml
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
    ```

- Renamed `--oneline` option to `--minify`. `--oneline` remains as an alias for `--minify`. ([#21](https://github.com/qryxip/cargo-equip/pull/21))

    With `--minify` option, cargo-equip removes spaces as much as possible.

    ```rust
    #[allow(clippy::deprecated_cfg_attr)]#[cfg_attr(rustfmt,rustfmt::skip)]pub mod factorial{pub fn factorial(n:u64)->u64{match n{0=>1,n=>n*factorial(n-1),}}}
    ```

## [0.3.4] - 2020-09-21Z

### Fixed

- Fixed the problem where `--remove comments` ruined code that contains tuple types. ([#19](https://github.com/qryxip/cargo-equip/pull/19))

## [0.3.3] - 2020-09-19Z

### Fixed

- `--remove` option now works for code that contains non-ASCII characters. ([#16](https://github.com/qryxip/cargo-equip/pull/16))

## [0.3.2] - 2020-09-17Z

### Added

- Added `--remove` option. ([#13](https://github.com/qryxip/cargo-equip/pull/13))

    Now you can remove

    - Items with `#[cfg(test)]`
    - Doc comments (`//! ..`, `/// ..`, `/** .. */`, `#[doc = ".."]`)
    - Comments (`// ..`, `/* .. */`)

    from the output.

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

## [0.3.1] - 2020-09-15Z

### Added

- Enabled bundling multiple libraries.

    ```rust
    #[cfg_attr(cargo_equip, cargo_equip::equip)]
    use ::{
        __lib1::{a::A, b::B, c::C},
        __lib2::{d::D, e::E, f::F},
    };
    ```

## [0.3.0] - 2020-09-03Z

### Added

- `cargo_equip_marker` is no longer required.

    ```rust
    #[cfg_attr(cargo_equip, cargo_equip::equip)]
    use ::__my_lib::{a::A, b::B, c::C};
    ```

### Changed

- Appends the `"# Bundled libralies"` section to existing doc comment.
- Changed the format of `"# Bundled libralies"`.

## [0.2.0] - 2020-09-02Z

### Changed

- Now it includes the used module list in the output.

### Fixed

- `--check` option will work with `{ path = ".." }` dependencies.
