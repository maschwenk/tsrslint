// Port of cmd/tsgolint/main.go + headless.go (headless mode only).

#[cfg(all(feature = "mimalloc", not(feature = "alloc-profile")))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("headless") => {}
        Some("--version") | Some("version") => {
            println!(
                "tsrslint {} (tsgolint headless protocol v2)",
                env!("CARGO_PKG_VERSION")
            );
            return;
        }
        _ => {
            eprintln!("usage: tsgolint headless [-fix] [-fix-suggestions] [-debug timings] < payload.json");
            std::process::exit(1);
        }
    }
    let rest = args[1..].to_vec();
    // Type checking recurses deeply; tsrs runs its checkers on 256 MiB-stack pool threads, and the
    // coordinating thread gets a large stack too (Go's goroutine stacks grow on demand).
    let status = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || {
            let status = tsrslint::linter::run_headless(&rest);
            tsrs_core::alloc_profile_dump();
            status
        })
        .unwrap()
        .join()
        .unwrap_or(1);
    std::process::exit(status);
}
