// Twin: a command-line program's `fn main` at src/main.rs that drives no PoC
// harness. It is a program entry point, not a test, and must stay unread.
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let total: u64 = args.iter().skip(1).filter_map(|a| a.parse().ok()).sum();
    assert!(total < u64::MAX);
    println!("{total}");
}
