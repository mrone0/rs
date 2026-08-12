fn main() {
    if let Err(error) = span::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
