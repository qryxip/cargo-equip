use cargo_equip::shell::Shell;
use insta::assert_snapshot;
use once_cell::sync::Lazy;
use std::{
    cell::RefCell,
    env,
    io::{self, Write},
    path::Path,
    rc::Rc,
    str,
    sync::{Mutex, MutexGuard},
};
use structopt::StructOpt as _;

// TODO:
// - arrayvec 0.5 ("error[E0433]: failed to resolve: use of undeclared crate or module `matches`")
// - arrayvec 0.7 (〃)
// - ascii ("Apache-2.0 / MIT")
// - either ("error[E0277]: the size for values of type `[i8]` cannot be known at compilation time")
// - if_chain (minification)
// - itertools (either)
// - memchr ("Unlicense/MIT")
// - smallvec (`#[deny(missing_docs)]`)
// - superslice ("LICENCE")

macro_rules! md5_snapshot_tests {
    ($($name:ident;)*) => {
        $(
            #[test]
            fn $name() -> anyhow::Result<()> {
                let output = format!("{:x}", md5::compute(snapshot_test(&stringify!($name).replace('_', "-"), LOCK.lock().unwrap())?));
                assert_snapshot!(stringify!($name), output);
                Ok(())
            }
        )*
    };
}

static LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

md5_snapshot_tests! {
    fixedbitset;
    lazy_static_with_macro_use;
    lazy_static_with_use;
    maplit_with_macro_use;
    maplit_with_use;
    multimap_with_macro_use;
    multimap_with_use;
    permutohedron;
    proconio_with_macro_use;
    rustc_hash;
    smallvec_with_macro_use;
    smallvec_with_use;
    strsim;
    whiteread;
}

fn snapshot_test(name: &str, _: MutexGuard<'_, ()>) -> anyhow::Result<String> {
    let stdout = Rc::new(RefCell::default());

    cargo_equip::run(
        cargo_equip::Opt::from_iter_safe(&[
            "",
            "equip",
            "--toolchain",
            &env::var("CARGO_EQUIP_TEST_NIGHTLY_TOOLCHAIN")
                .unwrap_or_else(|_| "nightly".to_owned()),
            "--resolve-cfgs",
            "--remove",
            "docs",
            "--minify",
            "libs",
            "--rustfmt",
            "--check",
            "--bin",
            &name.replace('_', "-"),
        ])?,
        cargo_equip::Context {
            cwd: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("solutions"),
            cache_dir: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("cache"),
            shell: &mut Shell::from_stdout(Box::new(Writer(stdout.clone()))),
        },
    )?;

    let stdout = String::from_utf8(Rc::try_unwrap(stdout).unwrap().into_inner())?;
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
