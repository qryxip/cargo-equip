use memoise::memoise;

fn main() {
    for i in 0..=10 {
        println!("{}", fib(i));
    }
}

#[memoise(n <= 10)]
fn fib(n: i64) -> i64 {
    if n == 0 || n == 1 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}
