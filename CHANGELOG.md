# Changelog

## [Unreleased]

### Added

- Now cargo-equip replaces `crate` paths in library code with `crate::extern_crate_name_in_main_crate`.

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
