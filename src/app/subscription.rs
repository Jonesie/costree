// SPDX-License-Identifier: MIT

use std::time::Duration;

use cosmic::iced::keyboard::key::Named;
use cosmic::iced::keyboard::{self, Key};
use cosmic::iced::{Subscription, futures};
use futures::SinkExt;

use super::{AppModel, Message};

pub(super) fn subscription(app: &AppModel) -> Subscription<Message> {
    let mut subscriptions = vec![cosmic::iced::event::listen_with(key_event_to_message)];

    if app.pending_branches > 0 || app.listing {
        subscriptions.push(Subscription::run(|| {
            cosmic::iced::stream::channel(1, |mut emitter: futures::channel::mpsc::Sender<_>| async move {
                let mut interval = tokio::time::interval(Duration::from_millis(150));
                loop {
                    interval.tick().await;
                    if emitter.send(Message::Tick).await.is_err() {
                        break;
                    }
                }
            })
        }));
    }

    Subscription::batch(subscriptions)
}

fn key_event_to_message(
    event: cosmic::iced::Event,
    status: cosmic::iced::event::Status,
    _window: cosmic::iced::window::Id,
) -> Option<Message> {
    if status == cosmic::iced::event::Status::Captured {
        return None;
    }

    let cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event else {
        return None;
    };

    match key {
        Key::Named(Named::F5) => Some(Message::Rescan),
        Key::Named(Named::Delete) => Some(Message::DeleteRequested),
        _ => None,
    }
}
