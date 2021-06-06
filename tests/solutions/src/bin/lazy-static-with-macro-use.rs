#[macro_use]
extern crate lazy_static as _;

fn main() {
    let _: i32 = *N;
}

lazy_static! {
    static ref N: i32 = 42;
}
