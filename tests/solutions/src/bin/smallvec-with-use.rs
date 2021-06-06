use smallvec::{smallvec, SmallVec};

fn main() {
    let _: SmallVec<[(); 0]> = smallvec![];
}
