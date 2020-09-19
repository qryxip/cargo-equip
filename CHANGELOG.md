# Changelog

## [Unreleased]

### Fixed

- `--remove` option now works for code that contains non-ASCII characters.

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
