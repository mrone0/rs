#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    if let Err(error) = span::open_gui() {
        report_error(&error);
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn report_error(error: &std::io::Error) {
    eprintln!("Span GUI error: {error}");
}

#[cfg(windows)]
fn report_error(_error: &std::io::Error) {}
