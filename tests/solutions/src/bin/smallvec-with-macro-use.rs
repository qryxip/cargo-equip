#[macro_use]
extern crate smallvec as _;

use smallvec::SmallVec;

fn main() {
    let _: SmallVec<[(); 0]> = smallvec![];
}
