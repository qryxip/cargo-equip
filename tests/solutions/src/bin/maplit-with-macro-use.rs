#[macro_use]
extern crate maplit as _;

fn main() {
    let _ = btreemap!(() => ());
    let _ = btreeset!(());
    let _ = hashmap!(() => ());
    let _ = hashset!(());
    assert_eq!(hashset!(2), convert_args!(keys = |x| x + 1, hashset!(1)));
}
