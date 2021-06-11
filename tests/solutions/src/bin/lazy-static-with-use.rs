#![deny(unused_imports)]

use lazy_static::lazy_static;

fn main() {
    let _: i32 = *N;
}

lazy_static! {
    static ref N: i32 = 42;
}
