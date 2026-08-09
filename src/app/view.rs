// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::path::PathBuf;

use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;

use crate::scanner::{self, Entry};

use super::{AppModel, Message};

const MUTED_TEXT: cosmic::iced::Color = cosmic::iced::Color::from_rgb(0.5, 0.5, 0.5);

/// While searching, every matching branch is force-expanded so results are
/// visible without manual clicking. A broad query (even a single common
/// letter) on a huge tree can match a large fraction of it, which without a
/// cap would try to build hundreds of thousands of widget rows in one
/// synchronous render and hang the whole UI. This bounds that regardless of
/// how many entries actually match.
const MAX_SEARCH_RESULTS: usize = 1000;

pub(super) fn view(app: &AppModel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();

    let can_delete = app
        .selected
        .as_ref()
        .is_some_and(|p| *p != app.scan_root);

    let scanning = app.listing || app.pending_branches > 0;

    let quick_root_labels: Vec<String> =
        app.quick_roots.iter().map(|(label, _)| label.clone()).collect();

    let title_content: Element<Message> = if app.title_editing {
        widget::row::with_capacity(3)
            .push(
                widget::text_input("Path to scan…", app.root_input.clone())
                    .on_input(Message::RootInputChanged)
                    .on_submit(|_| Message::RootSubmitted)
                    .width(Length::Fixed(420.0)),
            )
            .push(
                widget::button::icon(widget::icon::from_name("object-select-symbolic"))
                    .on_press(Message::RootSubmitted),
            )
            .push(
                widget::button::icon(widget::icon::from_name("window-close-symbolic"))
                    .on_press(Message::TitleEditCancelled),
            )
            .spacing(6)
            .align_y(Alignment::Center)
            .into()
    } else {
        widget::row::with_capacity(2)
            .push(widget::text::title3(app.scan_root.to_string_lossy().into_owned()))
            .push(
                widget::button::icon(widget::icon::from_name("document-edit-symbolic"))
                    .on_press(Message::EditTitle),
            )
            .spacing(6)
            .align_y(Alignment::Center)
            .into()
    };

    let title_row = widget::row::with_capacity(6)
        .push(widget::dropdown(
            quick_root_labels,
            None,
            Message::QuickRootSelected,
        ))
        .push(title_content)
        .push(widget::Space::new().width(Length::Fill))
        .push(
            toolbar_button("process-stop-symbolic", "Cancel")
                .on_press_maybe(scanning.then_some(Message::CancelScan)),
        )
        .push(toolbar_button("view-refresh-symbolic", "Refresh (F5)").on_press(Message::Rescan))
        .push(
            toolbar_button("user-trash-symbolic", "Delete (Del)")
                .on_press_maybe(can_delete.then_some(Message::DeleteRequested)),
        )
        .align_y(Alignment::Center)
        .spacing(spacing.space_s);

    let controls_row = widget::row::with_capacity(3)
        .push(
            widget::checkbox(app.hide_dotfiles)
                .label("Hide dotfiles")
                .on_toggle(Message::HideDotfilesToggled),
        )
        .push(
            widget::search_input("Search…", app.search_query.clone())
                .on_input(Message::SearchChanged)
                .width(Length::FillPortion(1)),
        )
        .push_maybe(app.searching.then(|| widget::text::caption("Searching…")))
        .align_y(Alignment::Center)
        .spacing(spacing.space_s);

    let toolbar = widget::container(
        widget::column::with_capacity(2)
            .push(title_row)
            .push(controls_row)
            .spacing(spacing.space_s),
    )
    .width(Length::Fill)
    .padding(spacing.space_s)
    .class(cosmic::theme::Container::Card);

    let body: Element<_> = if app.listing && app.root.is_none() {
        widget::container(widget::text::body("Listing directory…"))
            .center(Length::Fill)
            .into()
    } else if let Some(root) = &app.root {
        let search_matches = app.search_results.as_ref();
        let window_id = app.core.main_window_id();
        let mut rows: Vec<Element<Message>> = Vec::new();
        render_entry(
            root,
            0,
            &app.expanded,
            &app.selected,
            app.hide_dotfiles,
            search_matches,
            window_id,
            &mut rows,
        );

        if let Some(matches) = search_matches {
            if matches.len() > rows.len() {
                rows.push(
                    widget::text::caption(format!(
                        "Showing the first {} matches — refine your search to see more.",
                        rows.len()
                    ))
                    .class(cosmic::theme::Text::Color(MUTED_TEXT))
                    .into(),
                );
            }
        }

        let mut list = widget::column::with_capacity(rows.len())
            .spacing(2)
            .padding([0.0, 16.0, 0.0, 0.0]);
        for row in rows {
            list = list.push(row);
        }

        widget::scrollable(list).height(Length::Fill).width(Length::Fill).into()
    } else {
        widget::container(widget::text::body("No data"))
            .center(Length::Fill)
            .into()
    };

    let body = widget::container(body)
        .padding(spacing.space_s)
        .height(Length::Fill)
        .width(Length::Fill);

    widget::column::with_capacity(2)
        .push(toolbar)
        .push(body)
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

pub(super) fn footer(app: &AppModel) -> Option<Element<'_, Message>> {
    let spacing = cosmic::theme::spacing();

    let percent = if app.total_branches == 0 {
        100
    } else {
        (((app.total_branches - app.pending_branches) as f32 / app.total_branches as f32)
            * 100.0) as u32
    };

    let operation = if app.listing {
        "Listing directory…".to_string()
    } else if app.pending_branches > 0 {
        scanner::current_scan_path().map_or_else(
            || "Scanning…".to_string(),
            |p| format!("Scanning {}", p.display()),
        )
    } else {
        "Idle".to_string()
    };

    let mut row = widget::row::with_capacity(3)
        .push(widget::text::caption(format!("{percent}%")))
        .push(widget::text::caption(operation).width(Length::Fill))
        .align_y(Alignment::Center)
        .spacing(spacing.space_s);

    if let Some(err) = &app.last_error {
        row = row.push(widget::text::caption(format!("Error: {err}")));
    }

    Some(
        widget::container(row)
            .width(Length::Fill)
            .padding(spacing.space_xxs)
            .class(cosmic::theme::Container::Card)
            .into(),
    )
}

pub(super) fn dialog(app: &AppModel) -> Option<Element<'_, Message>> {
    if let Some(path) = &app.confirm_delete {
        let is_dir = app
            .root
            .as_ref()
            .and_then(|root| scanner::find_entry(root, path))
            .is_some_and(|entry| entry.is_dir);
        let kind = if is_dir { "folder" } else { "file" };

        return Some(
            widget::dialog()
                .title("Delete permanently?")
                .body(format!(
                    "This will permanently delete the {kind} \"{}\". This cannot be undone.",
                    path.display()
                ))
                .primary_action(
                    widget::button::destructive("Delete").on_press(Message::DeleteConfirmed),
                )
                .secondary_action(
                    widget::button::standard("Cancel").on_press(Message::DeleteCancelled),
                )
                .into(),
        );
    }

    if let Some((_, name)) = &app.rename_target {
        return Some(
            widget::dialog()
                .title("Rename")
                .control(
                    widget::text_input("Name", name.clone())
                        .on_input(Message::RenameInputChanged)
                        .on_submit(|_| Message::RenameConfirmed),
                )
                .primary_action(
                    widget::button::suggested("Rename").on_press(Message::RenameConfirmed),
                )
                .secondary_action(
                    widget::button::standard("Cancel").on_press(Message::RenameCancelled),
                )
                .into(),
        );
    }

    None
}

fn toolbar_button<'a>(icon_name: &'static str, label: &'a str) -> widget::button::Button<'a, Message> {
    widget::button::custom(
        widget::row::with_capacity(2)
            .push(widget::icon::from_name(icon_name))
            .push(widget::text::body(label))
            .spacing(6)
            .align_y(Alignment::Center),
    )
}

fn context_menu_entry(label: &'static str, message: Message) -> Element<'static, Message> {
    widget::menu::menu_button(vec![widget::text::body(label).into()])
        .on_press(message)
        .into()
}

fn build_context_menu(path: PathBuf, is_dir: bool) -> Vec<widget::menu::Tree<Message>> {
    let open_label = if is_dir { "Open in Files" } else { "Open" };
    vec![
        widget::menu::Tree::new(context_menu_entry(
            open_label,
            Message::OpenInFiles(path.clone()),
        )),
        widget::menu::Tree::new(context_menu_entry(
            "Rename",
            Message::RenameRequested(path.clone()),
        )),
        widget::menu::Tree::new(context_menu_entry(
            "Delete",
            Message::DeleteContextRequested(path),
        )),
    ]
}

/// Flattens the visible portion of the tree (respecting which directories
/// are expanded) into a list of renderable rows.
fn render_entry<'a>(
    entry: &'a Entry,
    depth: u16,
    expanded: &HashSet<PathBuf>,
    selected: &Option<PathBuf>,
    hide_dotfiles: bool,
    search_matches: Option<&HashSet<PathBuf>>,
    window_id: Option<cosmic::iced::window::Id>,
    rows: &mut Vec<Element<'a, Message>>,
) {
    if hide_dotfiles && depth > 0 && entry.name.starts_with('.') {
        return;
    }

    if let Some(matches) = search_matches {
        if !matches.contains(&entry.path) {
            return;
        }
        if rows.len() >= MAX_SEARCH_RESULTS {
            return;
        }
    }

    let indent = f32::from(depth) * 20.0;
    // While searching, force every matching branch open so results are visible
    // without the user having to manually expand down to them.
    let is_expanded = search_matches.is_some() || expanded.contains(&entry.path);
    let is_selected = selected.as_deref() == Some(entry.path.as_path());
    let unscanned = entry.is_dir && !entry.scanned;
    let text_class = if unscanned {
        cosmic::theme::Text::Color(MUTED_TEXT)
    } else {
        cosmic::theme::Text::Default
    };

    let marker_text = if entry.is_dir {
        if is_expanded { "▾" } else { "▸" }
    } else {
        " "
    };

    let marker: Element<'a, Message> = if entry.is_dir {
        widget::mouse_area(widget::text::body(marker_text).class(text_class))
            .on_press(Message::ToggleExpand(entry.path.clone()))
            .into()
    } else {
        widget::text::body(marker_text).into()
    };

    let size_text = if unscanned {
        String::new()
    } else {
        scanner::human_size(entry.size)
    };

    let content = widget::row::with_capacity(2)
        .push(widget::text::body(entry.name.clone()).class(text_class).width(Length::Fill))
        .push(widget::text::body(size_text).class(text_class))
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let mut selectable = widget::mouse_area(content)
        .on_press(Message::Select(entry.path.clone()))
        .on_right_press(Message::Select(entry.path.clone()));
    if entry.is_dir {
        selectable = selectable.on_double_click(Message::ToggleExpand(entry.path.clone()));
    }

    let row_inner = widget::row::with_capacity(3)
        .push(widget::Space::new().width(indent))
        .push(marker)
        .push(selectable)
        .spacing(6)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let row_container = widget::container(row_inner).width(Length::Fill).class(
        if is_selected {
            cosmic::theme::Container::Primary
        } else {
            cosmic::theme::Container::Transparent
        },
    );

    let mut with_menu = widget::context_menu(
        row_container,
        Some(build_context_menu(entry.path.clone(), entry.is_dir)),
    )
    .on_surface_action(Message::Surface);
    if let Some(id) = window_id {
        with_menu = with_menu.window_id(id);
    }

    rows.push(with_menu.into());

    if entry.is_dir && is_expanded {
        for child in &entry.children {
            render_entry(
                child,
                depth + 1,
                expanded,
                selected,
                hide_dotfiles,
                search_matches,
                window_id,
                rows,
            );
        }
    }
}
