mod db;
mod fonts;
mod macos;
mod scanner;
mod strings;
mod thumbnail_cache;
#[cfg(test)]
mod utils;
mod view_model;

mod slint_shell;

fn main() -> Result<(), slint::PlatformError> {
    slint_shell::run()
}
