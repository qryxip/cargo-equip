use insta::assert_snapshot;
use std::{process::Output, str};

#[test]
fn helps() -> anyhow::Result<()> {
    assert_snapshot!("short", run("-h")?);
    assert_snapshot!("long", run("--help")?);
    Ok(())
}

fn run(flag: &str) -> anyhow::Result<String> {
    let assert = assert_cmd::Command::cargo_bin("cargo-equip")?
        .args(["equip", flag])
        .assert()
        .success();
    let Output { stdout, .. } = assert.get_output();
    Ok(str::from_utf8(stdout)?.replacen(env!("CARGO_PKG_VERSION"), "<version>", 1))
}
