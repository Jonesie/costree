// SPDX-License-Identifier: MIT

mod app;
mod config;
mod scanner;

/// Set from the `version` field in `Cargo.toml` at compile time, which the
/// release workflow rewrites to match the pushed tag before building — see
/// `.github/workflows/release.yml`.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> cosmic::iced::Result {
    if std::env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("costree {VERSION}");
        return Ok(());
    }

    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(480.0)
            .min_height(320.0),
    );

    cosmic::app::run::<app::AppModel>(settings, ())
}
