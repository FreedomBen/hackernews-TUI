use super::find_bar::{self, FindSignal, FindStateRef};
use super::{async_view, help_view::HasHelpView, link_dialog, traits::*, utils};
use crate::prelude::*;

/// ArticleView is a View used to display the content of a web page in reader mode
pub struct ArticleView {
    article: Article,
    links: Vec<String>,
    width: usize,

    view: ScrollView<LinearLayout>,

    raw_command: String,

    find_state: FindStateRef,
    /// The un-highlighted content as produced by the last re-parse.
    /// `apply_find_query` layers highlights on top of this; `clear`
    /// restores the displayed text back to it.
    base_content: Option<StyledString>,
}

impl ViewWrapper for ArticleView {
    wrap_impl!(self.view: ScrollView<LinearLayout>);

    fn wrap_layout(&mut self, size: Vec2) {
        if self.width != size.x {
            // got a new width since the last time the article view is rendered,
            // re-parse the article using the new width

            self.width = size.x;

            match self.article.parse(self.width.saturating_sub(5)) {
                Ok(result) => {
                    self.base_content = Some(result.content.clone());
                    self.set_article_content(result.content);
                    self.links = result.links;
                    // A fresh parse wipes any existing highlights; re-apply
                    // them from the active query if a find session is live.
                    let query = self.find_state.borrow().query.clone();
                    if !query.is_empty() {
                        self.apply_find_query(&query);
                    }
                }
                Err(err) => {
                    warn!("failed to parse the article: {}", err);
                }
            }
        }

        self.process_find_signal();
        self.with_view_mut(|v| v.layout(size));
    }

    fn wrap_take_focus(&mut self, _: Direction) -> Result<EventResult, CannotFocus> {
        Ok(EventResult::Consumed(None))
    }
}

impl ArticleView {
    pub fn new(article: Article, find_state: FindStateRef) -> Self {
        let component_style = &config::get_config_theme().component_style;
        let unknown = "[unknown]".to_string();
        let desc = format!(
            "by: {}, date_published: {}",
            article.author.as_ref().unwrap_or(&unknown),
            article.date_published.as_ref().unwrap_or(&unknown),
        );

        let view = LinearLayout::vertical()
            .child(TextView::new(&article.title).center().full_width())
            .child(
                TextView::new(StyledString::styled(desc, component_style.metadata))
                    .center()
                    .full_width(),
            )
            .child(PaddedView::lrtb(1, 1, 1, 1, TextView::new("")))
            .scrollable();

        ArticleView {
            article,
            links: vec![],
            width: 0,

            view,
            raw_command: "".to_string(),

            find_state,
            base_content: None,
        }
    }

    /// Update the content of the article
    pub fn set_article_content(&mut self, new_content: StyledString) {
        self.view
            .get_inner_mut()
            .get_child_mut(2)
            .expect("The article view should have 3 children")
            .downcast_mut::<PaddedView<TextView>>()
            .expect("The 3rd child of the article view should be a padded text view")
            .get_inner_mut()
            .set_content(new_content)
    }

    /// Poll the shared find state and apply any pending signal. The
    /// article view only supports highlighting; jump-to-match is a
    /// follow-up (scrolling a single large `TextView` to a byte offset
    /// needs row-offset plumbing that doesn't exist yet).
    fn process_find_signal(&mut self) {
        let signal = self.find_state.borrow_mut().pending.take();
        match signal {
            Some(FindSignal::Update) => {
                let query = self.find_state.borrow().query.clone();
                self.apply_find_query(&query);
            }
            Some(FindSignal::Clear) => {
                self.clear_find_highlights();
                let mut state = self.find_state.borrow_mut();
                state.query.clear();
                state.match_ids.clear();
            }
            Some(FindSignal::JumpNext) | Some(FindSignal::JumpPrev) => {
                // no-op: jump requires row-offset scrolling, not yet wired
            }
            None => {}
        }
    }

    fn apply_find_query(&mut self, query: &str) {
        let Some(base) = self.base_content.clone() else {
            return;
        };
        let style: Style = config::get_config_theme()
            .component_style
            .matched_highlight
            .into();
        let (highlighted, _count) = find_bar::highlight_matches(&base, query, style);
        self.set_article_content(highlighted);
    }

    fn clear_find_highlights(&mut self) {
        if let Some(base) = self.base_content.clone() {
            self.set_article_content(base);
        }
    }

    inner_getters!(self.view: ScrollView<LinearLayout>);
}

impl ScrollViewContainer for ArticleView {
    type ScrollInner = LinearLayout;

    fn get_inner_scroll_view(&self) -> &ScrollView<LinearLayout> {
        self.get_inner()
    }

    fn get_inner_scroll_view_mut(&mut self) -> &mut ScrollView<LinearLayout> {
        self.get_inner_mut()
    }
}

fn construct_article_main_view(
    client: &'static client::HNClient,
    article: Article,
) -> OnEventView<ArticleView> {
    let is_suffix_key = |c: &Event| -> bool {
        let article_view_keymap = config::get_article_view_keymap();
        article_view_keymap.open_link_in_browser.has_event(c)
            || article_view_keymap.open_link_in_article_view.has_event(c)
    };

    let article_view_keymap = config::get_article_view_keymap().clone();
    let find_state = find_bar::FindState::new_ref();
    let find_state_for_key = find_state.clone();

    OnEventView::new(ArticleView::new(article, find_state))
        .on_pre_event_inner(EventTrigger::from_fn(|_| true), move |s, e| {
            match *e {
                Event::Char(c) if c.is_ascii_digit() => {
                    s.raw_command.push(c);
                }
                _ => {
                    if !is_suffix_key(e) {
                        s.raw_command.clear();
                    }
                }
            };
            None
        })
        .on_pre_event_inner(article_view_keymap.open_link_dialog, move |s, _| {
            Some(EventResult::with_cb({
                let links = s.links.clone();
                move |s| {
                    s.add_layer(link_dialog::get_link_dialog(client, &links));
                }
            }))
        })
        .on_pre_event_inner(article_view_keymap.open_link_in_browser, |s, _| {
            match s.raw_command.parse::<usize>() {
                Ok(num) => {
                    s.raw_command.clear();
                    utils::open_ith_link_in_browser(&s.links, num)
                }
                Err(_) => None,
            }
        })
        .on_pre_event_inner(
            article_view_keymap.open_link_in_article_view,
            move |s, _| match s.raw_command.parse::<usize>() {
                Ok(num) => {
                    s.raw_command.clear();
                    utils::open_ith_link_in_article_view(client, &s.links, num)
                }
                Err(_) => None,
            },
        )
        .on_pre_event_inner(article_view_keymap.open_article_in_browser, |s, _| {
            utils::open_url_in_browser(&s.article.url);
            Some(EventResult::Consumed(None))
        })
        .on_pre_event(config::get_global_keymap().open_help_dialog.clone(), |s| {
            s.add_layer(ArticleView::construct_on_event_help_view())
        })
        // Open the find-on-page dialog. Highlight-only; no match jump.
        .on_pre_event(article_view_keymap.find_in_view.clone(), move |s| {
            {
                let mut state = find_state_for_key.borrow_mut();
                state.query.clear();
                state.pending = Some(FindSignal::Clear);
            }
            s.add_layer(find_bar::construct_find_dialog(find_state_for_key.clone()));
        })
        .on_scroll_events()
}

/// Construct an article view of an article
pub fn construct_article_view(client: &'static client::HNClient, article: Article) -> impl View {
    let desc = format!("Article View - {}", article.title);
    let main_view = construct_article_main_view(client, article).full_height();

    let mut view = LinearLayout::vertical()
        .child(utils::construct_view_title_bar(&desc))
        .child(main_view)
        .child(utils::construct_footer_view::<ArticleView>());
    view.set_focus_index(1)
        .unwrap_or(EventResult::Consumed(None));

    view
}

/// Retrieve an article from a given `url` and construct an article view of that article
pub fn construct_and_add_new_article_view(
    client: &'static client::HNClient,
    s: &mut Cursive,
    url: &str,
) {
    let async_view = async_view::construct_article_view_async(client, s, url);
    s.screen_mut().add_transparent_layer(Layer::new(async_view))
}
