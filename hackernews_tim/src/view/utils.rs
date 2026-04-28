use std::rc::Rc;

use super::{article_view, comment_view, help_view, search_view, story_view};
use crate::prelude::*;

/// HN story-tag tabs rendered in the nav strip, in F1..F5 order. Story
/// view also uses this for tag-cycling.
pub static STORY_TAGS: [&str; 5] = ["front_page", "story", "ask_hn", "show_hn", "job"];

/// Which entry of the global nav strip a view wants highlighted as
/// "you are here". Views that aren't anchored to a nav target (article
/// view, generic comment view) pass `None`.
#[derive(Debug, Clone, Copy)]
pub enum NavTarget {
    None,
    StoryTag(&'static str),
    MyThreads,
    Search,
}

/// Focusable cell inside the nav strip. Renders a styled label with the
/// reverse effect when focused, and fires a callback on Enter (or
/// click), mirroring the matching F-key shortcut.
struct NavLink {
    label: StyledString,
    width: usize,
    on_select: Rc<dyn Fn(&mut Cursive)>,
}

impl NavLink {
    fn new(label: StyledString, on_select: impl Fn(&mut Cursive) + 'static) -> Self {
        let width = label.width();
        Self {
            label,
            width,
            on_select: Rc::new(on_select),
        }
    }
}

impl View for NavLink {
    fn draw(&self, printer: &Printer) {
        if printer.focused {
            printer.with_effect(Effect::Reverse, |p| {
                p.print_styled((0, 0), &self.label);
            });
        } else {
            printer.print_styled((0, 0), &self.label);
        }
    }

    fn required_size(&mut self, _: Vec2) -> Vec2 {
        Vec2::new(self.width, 1)
    }

    fn take_focus(&mut self, _: Direction) -> std::result::Result<EventResult, CannotFocus> {
        Ok(EventResult::Consumed(None))
    }

    fn on_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Key(Key::Enter) => {
                let cb = self.on_select.clone();
                EventResult::with_cb(move |s| cb(s))
            }
            Event::Mouse {
                event: MouseEvent::Release(MouseButton::Left),
                position,
                offset,
            } if position.fits_in_rect(offset, Vec2::new(self.width, 1)) => {
                let cb = self.on_select.clone();
                EventResult::with_cb(move |s| cb(s))
            }
            _ => EventResult::Ignored,
        }
    }

    fn important_area(&self, _: Vec2) -> Rect {
        Rect::from_size((0, 0), (self.width, 1))
    }
}

/// Construct a simple footer view
pub fn construct_footer_view<T: help_view::HasHelpView>() -> impl View {
    LinearLayout::horizontal()
        .child(
            TextView::new(StyledString::styled(
                "Hacker News Terminal UI - made by AOME ©",
                config::get_config_theme().component_style.bold,
            ))
            .align(align::Align::bot_center())
            .full_width(),
        )
        .child(
            LinearLayout::horizontal()
                .child(Button::new_raw(
                    format!("[{}: help] ", config::get_global_keymap().open_help_dialog),
                    |s| s.add_layer(T::construct_on_event_help_view()),
                ))
                .child(Button::new_raw("[back] ", |s| {
                    if s.screen_mut().len() > 1 {
                        s.pop_layer();
                    } else {
                        s.quit();
                    }
                }))
                .child(Button::new_raw("[quit] ", |s| s.quit())),
        )
}

/// Build the "username (karma)" styled text rendered at the right edge of a
/// title bar, or an empty string when there's no logged-in user. The HN
/// website shows the same thing in its top-right nav area.
pub fn build_user_info_text(style: Style) -> StyledString {
    match client::get_user_info() {
        None => StyledString::new(),
        Some(info) => {
            let text = match info.karma {
                Some(k) => format!(" {} ({}) ", info.username, k),
                None => format!(" {} ", info.username),
            };
            StyledString::styled(text, style)
        }
    }
}

/// " | " separator between nav-strip cells. Marked `no_wrap` so the
/// surrounding `LinearLayout::horizontal` doesn't try to break the
/// strip across two rows when the title bar gets squeezed.
fn nav_separator(style: Style) -> impl View {
    TextView::new(StyledString::styled(" | ", style)).no_wrap()
}

/// Build the global nav strip as a focusable horizontal row of
/// `NavLink`s separated by " | ". Each link runs the same callback
/// the matching F-key would; the entry matching `active` is rendered
/// in the "you are here" style. Wrapped in `OnEventView` so `h`/`l`
/// alias `Left`/`Right` for vim-style in-strip navigation.
fn construct_nav_strip(
    client: &'static client::HNClient,
    active: NavTarget,
    sort_suffix: &str,
) -> impl View {
    let theme = config::get_config_theme();
    let style: Style = theme.component_style.title_bar.into();
    let active_style = style.combine(theme.component_style.current_story_tag);

    let mut row = LinearLayout::horizontal();

    let mut prefix = StyledString::styled(
        "[Y]",
        style.combine(ColorStyle::front(theme.palette.light_white)),
    );
    prefix.append_styled(" Hacker News", style);
    row.add_child(TextView::new(prefix).no_wrap());

    for (i, tag) in STORY_TAGS.iter().enumerate() {
        row.add_child(nav_separator(style));
        let is_active = matches!(active, NavTarget::StoryTag(t) if t == *tag);
        let label_text = if is_active {
            format!("{}.{tag}{sort_suffix}", i + 1)
        } else {
            format!("{}.{tag}", i + 1)
        };
        let label = StyledString::styled(label_text, if is_active { active_style } else { style });
        let tag_static: &'static str = tag;
        row.add_child(NavLink::new(label, move |s| {
            story_view::construct_and_add_new_story_view(
                s,
                client,
                tag_static,
                if tag_static == "story" || tag_static == "job" {
                    client::StorySortMode::Date
                } else {
                    client::StorySortMode::None
                },
                0,
                client::StoryNumericFilters::default(),
                false,
            );
        }));
    }

    row.add_child(nav_separator(style));
    let is_threads_active = matches!(active, NavTarget::MyThreads);
    let threads_label = StyledString::styled(
        "6.threads",
        if is_threads_active {
            active_style
        } else {
            style
        },
    );
    row.add_child(NavLink::new(
        threads_label,
        move |s| match client::get_user_info() {
            Some(info) => {
                comment_view::construct_and_add_new_threads_view(
                    s,
                    client,
                    info.username.clone(),
                    0,
                );
            }
            None => {
                s.add_layer(
                    Dialog::info(
                        "Log in first (press `L`) to view your comments on \
                         Hacker News.",
                    )
                    .title("Not logged in"),
                );
            }
        },
    ));

    row.add_child(nav_separator(style));
    let is_search_active = matches!(active, NavTarget::Search);
    let search_label = StyledString::styled(
        "search (^S)",
        if is_search_active {
            active_style
        } else {
            style
        },
    );
    row.add_child(NavLink::new(search_label, move |s| {
        search_view::construct_and_add_new_search_view(s, client);
    }));

    row.add_child(nav_separator(style));

    OnEventView::new(row)
        .on_pre_event_inner(Event::Char('h'), |inner, _| {
            Some(inner.on_event(Event::Key(Key::Left)))
        })
        .on_pre_event_inner(Event::Char('l'), |inner, _| {
            Some(inner.on_event(Event::Key(Key::Right)))
        })
}

/// The nav-strip-only top bar used by the story view. Other views render
/// the strip plus a description row via [`construct_view_title_bar`].
pub fn construct_story_view_top_bar(
    client: &'static client::HNClient,
    active_tag: &'static str,
    sort_mode: client::StorySortMode,
) -> impl View {
    let suffix = match sort_mode {
        client::StorySortMode::None => "",
        client::StorySortMode::Date => " (by_date)",
        client::StorySortMode::Points => " (by_point)",
    };
    let style = config::get_config_theme().component_style.title_bar;
    let user_info = build_user_info_text(style.into());

    PaddedView::lrtb(
        0,
        0,
        0,
        1,
        Layer::with_color(
            LinearLayout::horizontal()
                .child(construct_nav_strip(
                    client,
                    NavTarget::StoryTag(active_tag),
                    suffix,
                ))
                .child(TextView::new(StyledString::new()).full_width())
                .child(TextView::new(user_info).no_wrap()),
            style.into(),
        ),
    )
}

/// Construct a view's title bar (nav strip + centered description).
/// Equivalent to [`construct_view_title_bar_with_nav`] with no nav
/// target highlighted.
pub fn construct_view_title_bar(client: &'static client::HNClient, desc: &str) -> impl View {
    construct_view_title_bar_with_nav(client, desc, NavTarget::None)
}

/// Two-row title bar: the global nav strip on top (with the matching
/// entry highlighted), and the per-view description centered below.
pub fn construct_view_title_bar_with_nav(
    client: &'static client::HNClient,
    desc: &str,
    nav: NavTarget,
) -> impl View {
    let style = config::get_config_theme().component_style.title_bar;
    let user_info = build_user_info_text(style.into());

    let nav_layer = Layer::with_color(
        LinearLayout::horizontal()
            .child(construct_nav_strip(client, nav, ""))
            .child(TextView::new(StyledString::new()).full_width())
            .child(TextView::new(user_info).no_wrap()),
        style.into(),
    );

    let desc_layer = Layer::with_color(
        TextView::new(StyledString::styled(desc, style))
            .h_align(align::HAlign::Center)
            .full_width(),
        style.into(),
    );

    LinearLayout::vertical().child(nav_layer).child(desc_layer)
}

/// Open a given url using a specific command
pub fn open_url_in_browser(url: &str) {
    if url.is_empty() {
        return;
    }

    let url = url.to_string();
    let url_open_command = &config::get_config().url_open_command;
    std::thread::spawn(move || {
        match std::process::Command::new(&url_open_command.command)
            .args(&url_open_command.options)
            .arg(&url)
            .output()
        {
            Err(err) => warn!(
                "failed to execute command `{} {}`: {}",
                url_open_command, url, err
            ),
            Ok(output) => {
                if !output.status.success() {
                    warn!(
                        "failed to execute command `{} {}`: {}",
                        url_open_command,
                        url,
                        std::str::from_utf8(&output.stderr).unwrap(),
                    )
                }
            }
        }
    });
}

/// open in article view the `i`-th link.
/// Note that the link index starts with `1`.
pub fn open_ith_link_in_article_view(
    client: &'static client::HNClient,
    links: &[String],
    i: usize,
) -> Option<EventResult> {
    if i > 0 && i <= links.len() {
        Some(EventResult::with_cb({
            let url = links[i - 1].clone();
            move |s| article_view::construct_and_add_new_article_view(client, s, &url)
        }))
    } else {
        Some(EventResult::Consumed(None))
    }
}

/// Resolve the 1-indexed `i`-th link in `links`. Returns `None` when `i`
/// is out of range (including the special case `i == 0`, which the typed-
/// prefix UI treats as "no number"). Pure — extracted so the bounds half
/// of [`open_ith_link_in_browser`] can be unit tested without launching
/// a browser.
pub fn nth_link(links: &[String], i: usize) -> Option<&str> {
    if i > 0 && i <= links.len() {
        Some(&links[i - 1])
    } else {
        None
    }
}

/// open in browser the `i`-th link.
/// Note that the link index starts with `1`.
pub fn open_ith_link_in_browser(links: &[String], i: usize) -> Option<EventResult> {
    if let Some(link) = nth_link(links, i) {
        open_url_in_browser(link);
    }
    Some(EventResult::Consumed(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nth_link_empty_slice_returns_none() {
        let links: Vec<String> = vec![];
        assert!(nth_link(&links, 0).is_none());
        assert!(nth_link(&links, 1).is_none());
    }

    #[test]
    fn nth_link_zero_index_returns_none() {
        // 1-indexed: 0 means "no number typed", which is invalid here.
        let links = vec!["https://a.example".to_string()];
        assert!(nth_link(&links, 0).is_none());
    }

    #[test]
    fn nth_link_in_range_returns_link() {
        let links = vec![
            "https://a.example".to_string(),
            "https://b.example".to_string(),
        ];
        assert_eq!(nth_link(&links, 1), Some("https://a.example"));
        assert_eq!(nth_link(&links, 2), Some("https://b.example"));
    }

    #[test]
    fn nth_link_past_end_returns_none() {
        let links = vec!["https://a.example".to_string()];
        assert!(nth_link(&links, 2).is_none());
        assert!(nth_link(&links, 99).is_none());
    }
}
