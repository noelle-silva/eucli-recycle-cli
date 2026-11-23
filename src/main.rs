fn main() {
    let exit_code = eucli_recycle::cli::run_from_env();
    std::process::exit(exit_code);
}
