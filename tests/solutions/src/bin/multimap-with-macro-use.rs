#[macro_use]
extern crate multimap as _;

use multimap::MultiMap;

fn main() {
    let _: MultiMap<(), ()> = multimap!();
}
