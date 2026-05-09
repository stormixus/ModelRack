mod fonts;
mod macos;
mod scanner;
mod strings;
mod thumbnail_cache;
mod utils;
mod view_model;

mod slint_shell;

fn main() -> Result<(), slint::PlatformError> {
    slint_shell::run()
}
