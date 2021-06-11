#![deny(unused_imports)]

use multimap::{multimap, MultiMap};

fn main() {
    let _: MultiMap<(), ()> = multimap!();
}
