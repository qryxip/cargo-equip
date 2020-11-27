# Changelog

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
