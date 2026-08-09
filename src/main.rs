// SPDX-License-Identifier: MIT

mod app;
mod config;
mod scanner;

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(480.0)
            .min_height(320.0),
    );

    cosmic::app::run::<app::AppModel>(settings, ())
}
