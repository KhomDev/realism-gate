#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    // A deeply nested test expression can recurse far into the lowering; the
    // default main-thread stack is 1 MiB on Windows. Run the checker on a
    // thread with a stack big enough for the deepest real PoC.
    std::thread::Builder::new()
        .name("realism-gate".to_string())
        .stack_size(64 * 1024 * 1024)
        .spawn(realism_gate::cli::main)
        .expect("spawn the checker thread")
        .join()
        .unwrap_or_else(|_| std::process::ExitCode::from(2))
}
