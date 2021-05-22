use cargo_equip::shell::Shell;
use indoc::indoc;
use insta::assert_snapshot;
use std::{
    cell::RefCell,
    env,
    io::{self, Write},
    rc::Rc,
    str,
};
use structopt::StructOpt as _;

macro_rules! md5_snapshot_test {
    ($name:expr, $src:expr, $dependencies:expr $(,)?) => {{
        let output = format!("{:x}", md5::compute(snapshot_test($src, $dependencies)?));
        assert_snapshot!($name, output);
        Ok(())
    }};
}

// error[E0433]: failed to resolve: use of undeclared crate or module `matches`
//#[test]
//fn arrayvec_0_5() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "arrayvec_0_5",
//        indoc! {r#"
//            use arrayvec::ArrayVec;
//
//            fn main() {
//                let _ = ArrayVec::<[(); 1]>::new();
//            }
//        "#},
//        indoc! {r#"
//            arrayvec = "=0.5.2"
//        "#},
//    )
//}

// error[E0433]: failed to resolve: use of undeclared crate or module `matches`
//#[test]
//fn arrayvec_0_7() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "arrayvec_0_7",
//        indoc! {r#"
//            use arrayvec::ArrayVec;
//
//            fn main() {
//                let _ = ArrayVec::<(), 1>::new();
//            }
//        "#},
//        indoc! {r#"
//            arrayvec = "=0.7.0"
//        "#},
//    )
//}

// Apache-2.0 / MIT
//            ^^^^^ invalid character(s)
//#[test]
//fn ascii() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "ascii",
//        indoc! {r#"
//            use ascii::AsciiStr;
//
//            fn main() {
//                let _ = AsciiStr::from_ascii("a").unwrap();
//            }
//        "#},
//        indoc! {r#"
//            ascii = "=1.0.0"
//        "#},
//    )
//}

// error[E0277]: the size for values of type `[i8]` cannot be known at compilation time
//#[test]
//fn either() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "either",
//        indoc! {r#"
//            use either::Either;
//
//            fn main() {
//                let _ = Either::<(), ()>::Left(());
//            }
//        "#},
//        indoc! {r#"
//            either = "=1.6.1"
//        "#},
//    )
//}

#[test]
fn fixedbitset() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "fixedbitset",
        indoc! {r#"
            use fixedbitset::FixedBitSet;

            fn main() {
                let _ = FixedBitSet::new();
            }
        "#},
        indoc! {r#"
            fixedbitset = "=0.4.0"
        "#},
    )
}

// fails to minify
//#[test]
//fn if_chain() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "if_chain",
//        indoc! {r#"
//            #[macro_use]
//            extern crate if_chain as _;
//
//            fn main() {
//                if_chain! {
//                    if true;
//                    then {}
//                }
//            }
//        "#},
//        indoc! {r#"
//            if_chain = "=1.0.1"
//        "#},
//    )
//}

//#[test]
//fn itertools() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "itertools",
//        indoc! {r#"
//            #[macro_use]
//            extern crate itertools as _;
//
//            fn main() {
//                let _ = iproduct!(0..=1, 0..=1);
//                let _ = izip!(0..=1, 0..=1);
//            }
//        "#},
//        indoc! {r#"
//            itertools = "=0.10.0"
//        "#},
//    )
//}

#[test]
fn lazy_static() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "lazy_static",
        indoc! {r#"
           #[macro_use]
           extern crate lazy_static as _;

            fn main() {
                let _: i32 = *N;
            }

            lazy_static! {
                static ref N: i32 = 42;
            }
        "#},
        indoc! {r#"
            lazy_static = "=1.4.0"
        "#},
    )
}

// Unlicense/MIT
//          ^^^^ invalid character(s)
//#[test]
//fn memchr() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "memchr",
//        indoc! {r#"
//            #[macro_use]
//            extern crate memchr as _;
//
//            fn main() {
//                assert_eq!(Some(0), memchr::memchr(b'a', b"a"));
//            }
//        "#},
//        indoc! {r#"
//            memchr = "=2.4.0"
//        "#},
//    )
//}

#[test]
fn maplit() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "maplit",
        indoc! {r#"
            #[macro_use]
            extern crate maplit as _;

            fn main() {
                let _ = btreemap!(() => ());
                let _ = btreeset!(());
                let _ = hashmap!(() => ());
                let _ = hashset!(());
                assert_eq!(
                    hashset!(2),
                    convert_args!(keys = |x| x + 1, hashset!(1)),
                );
            }
        "#},
        indoc! {r#"
            maplit = "=1.0.2"
        "#},
    )
}

#[test]
fn multimap() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "multimap",
        indoc! {r#"
            #[macro_use]
            extern crate multimap as _;

            use multimap::MultiMap;

            fn main() {
                let _: MultiMap<(), ()> = multimap!();
            }
        "#},
        indoc! {r#"
            multimap = { version = "=0.8.3", default-features = false }
        "#},
    )
}

#[test]
fn permutohedron() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "permutohedron",
        indoc! {r#"
            fn main() {
                permutohedron::heap_recursive::<(), _, _>(&mut [], |_| ());
            }
        "#},
        indoc! {r#"
            permutohedron = "=0.2.4"
        "#},
    )
}

#[test]
fn proconio() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "proconio",
        indoc! {r#"
           #[macro_use]
           extern crate proconio as _;

            fn main() {
                input!(_: i32);
            }
        "#},
        indoc! {r#"
            proconio = "=0.4.3"
        "#},
    )
}

//#[test]
//fn proconio_with_derive() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "proconio_with_derive",
//        indoc! {r#"
//           #[macro_use]
//           extern crate proconio as _;
//
//            #[fastout]
//            fn main() {
//                input!(_: i32);
//            }
//        "#},
//        indoc! {r#"
//            proconio = { version = "=0.4.3", features = ["derive"] }
//        "#},
//    )
//}

#[test]
fn rustc_hash() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "rustc_hash",
        indoc! {r#"
            use rustc_hash::FxHashMap;

            fn main() {
                let _ = FxHashMap::<(), ()>::default();
            }
        "#},
        indoc! {r#"
            rustc-hash = "=1.1.0"
        "#},
    )
}

#[test]
fn smallvec() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "smallvec",
        indoc! {r#"
            #[macro_use]
            extern crate smallvec as _;

            use smallvec::SmallVec;

            fn main() {
                let _: SmallVec<[(); 1]> = smallvec![];
            }
        "#},
        indoc! {r#"
            smallvec = "=1.6.1"
        "#},
    )
}

#[test]
fn strsim() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "strsim",
        indoc! {r#"
            fn main() {
                assert_eq!(Ok(1), strsim::hamming("abc", "abd"));
            }
        "#},
        indoc! {r#"
            strsim = "=0.10.0"
        "#},
    )
}

// "LICENCE"
// https://github.com/alkis/superslice-rs
//#[test]
//fn superslice() -> anyhow::Result<()> {
//    md5_snapshot_test!(
//        "superslice",
//        indoc! {r#"
//            use superslice::Ext as _;
//
//            fn main() {
//                [()].apply_permutation(&mut [0]);
//            }
//        "#},
//        indoc! {r#"
//            superslice = "=1.0.0"
//        "#},
//    )
//}

#[test]
fn whiteread() -> anyhow::Result<()> {
    md5_snapshot_test!(
        "whiteread",
        indoc! {r#"
            use whiteread::Reader;

            fn main() {
                let _: i32 = Reader::from_stdin_naive().p();
            }
        "#},
        indoc! {r#"
            whiteread = "=0.5.0"
        "#},
    )
}

fn snapshot_test(src: &str, dependencies: &str) -> anyhow::Result<String> {
    let tempdir = tempfile::Builder::new()
        .prefix("cargo-equip-tests-snapshots-")
        .tempdir()?;

    let mut manifest = indoc! {r#"
        [package]
        name = "snapshot"
        version = "0.0.0"
        edition = "2018"
        authors = ["Ryo Yamashita <qryxip@gmail.com>"]
    "#}
    .parse::<toml_edit::Document>()
    .unwrap();

    manifest["dependencies"] = dependencies.parse::<toml_edit::Document>()?.root;

    xshell::mkdir_p(tempdir.path().join("src"))?;
    xshell::write_file(tempdir.path().join("Cargo.toml"), manifest.to_string())?;
    xshell::write_file(tempdir.path().join("src").join("main.rs"), src)?;

    let stdout = Rc::new(RefCell::default());

    cargo_equip::run(
        cargo_equip::Opt::from_iter_safe(&[
            "",
            "equip",
            "--toolchain",
            &env::var("CARGO_EQUIP_TEST_NIGHTLY_TOOLCHAIN")
                .unwrap_or_else(|_| "nightly".to_owned()),
            "--resolve-cfgs",
            "--minify",
            "libs",
            "--rustfmt",
            "--check",
        ])?,
        cargo_equip::Context {
            cwd: tempdir.path().to_owned(),
            shell: &mut Shell::from_stdout(Box::new(Writer(stdout.clone()))),
        },
    )?;

    let stdout = String::from_utf8(Rc::try_unwrap(stdout).unwrap().into_inner())?;

    tempdir.close()?;
    return Ok(stdout);

    struct Writer(Rc<RefCell<Vec<u8>>>);

    impl Write for Writer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.borrow_mut().flush()
        }
    }
}
