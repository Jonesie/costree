// SPDX-License-Identifier: MIT

use costree::{VERSION, app};

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
