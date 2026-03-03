fn main() {
    if let Err(e) = claude_code_swap::cli::run() {
        eprintln!("ccs: error: {e}");
        std::process::exit(1);
    }
}
