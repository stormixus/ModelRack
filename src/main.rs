mod fonts;
mod macos;
mod scanner;
mod strings;
mod utils;
mod view_model;

mod slint_shell;

fn main() -> Result<(), slint::PlatformError> {
    slint_shell::run()
}
