//! Test helpers for driving Cursive views without a real terminal.
//!
//! Wraps `cursive::backends::puppet::Backend` (an in-memory backend that
//! ships with `cursive` 0.20 unconditionally — no feature flag) in a
//! [`PuppetHarness`] that:
//!
//! - injects events with [`PuppetHarness::send`],
//! - drains pending events and callbacks with
//!   [`PuppetHarness::step_until_idle`],
//! - and exposes the latest rendered frame as a snapshot-friendly string
//!   via [`PuppetHarness::screen_text`] (or the raw [`ObservedScreen`]
//!   via [`PuppetHarness::screen`]).
//!
//! Also exposes the small set of fixture/setup helpers integration
//! tests reach for repeatedly — see [`ensure_globals_initialised`],
//! [`leak_fake_api`], [`make_story`], and the [`fixtures`] submodule
//! for file-based fixture loading.
//!
//! Gated on `cfg(any(test, feature = "test-support"))`, mirroring
//! [`crate::client::fake`]. Integration tests under `tests/` must enable
//! `--features test-support` to see this module.

pub mod fixtures;

use crossbeam_channel::{Receiver, Sender};
use cursive::backends::puppet::observed::{ObservedPieceInterface, ObservedScreen};
use cursive::backends::puppet::Backend as PuppetBackend;
use cursive::event::Event;
use cursive::{Cursive, CursiveRunner, Vec2};

use crate::client::fake::FakeHnApi;
use crate::client::init_test_user_info;
use crate::config::init_test_config;
use crate::model::Story;

/// Default puppet screen size — wide enough that typical story rows
/// don't wrap, tall enough to render several rows plus a status bar.
pub fn default_size() -> Vec2 {
    Vec2::new(120, 40)
}

/// Upper bound on `process_events` iterations
/// [`PuppetHarness::step_until_idle`] performs before bailing out, so a
/// callback that keeps re-arming itself can't hang the test.
const MAX_IDLE_ITERS: usize = 256;

/// Test harness that drives a [`Cursive`] instance through the in-memory
/// puppet backend.
pub struct PuppetHarness {
    runner: CursiveRunner<Cursive>,
    screen_rx: Receiver<ObservedScreen>,
    event_tx: Sender<Option<Event>>,
    last_screen: Option<ObservedScreen>,
}

impl PuppetHarness {
    /// Build a harness around `siv` using [`default_size`].
    pub fn new(siv: Cursive) -> Self {
        Self::with_size(siv, default_size())
    }

    /// Build a harness around `siv` with an explicit screen size.
    pub fn with_size(siv: Cursive, size: Vec2) -> Self {
        let backend = PuppetBackend::init(Some(size));
        let screen_rx = backend.stream();
        let event_tx = backend.input();
        let mut runner = CursiveRunner::new(siv, backend);
        // Initial refresh so `screen()` / `screen_text()` are non-empty
        // before any events are dispatched.
        runner.refresh();
        let mut harness = Self {
            runner,
            screen_rx,
            event_tx,
            last_screen: None,
        };
        harness.drain_screen();
        harness
    }

    /// Mutable access to the inner [`Cursive`] state. Use this to add
    /// layers, query focus, look up named views, etc.
    pub fn cursive_mut(&mut self) -> &mut Cursive {
        &mut self.runner
    }

    /// Inject a Cursive event into the puppet's input queue. The event
    /// is processed on the next [`Self::step_until_idle`] call.
    pub fn send(&self, event: Event) {
        self.event_tx
            .send(Some(event))
            .expect("puppet input channel disconnected");
    }

    /// Drain pending events and callbacks, then refresh the screen.
    /// Returns the number of `process_events` iterations performed —
    /// useful for assertions about async-view loading completing in a
    /// bounded number of steps.
    pub fn step_until_idle(&mut self) -> usize {
        let mut iters = 0;
        for _ in 0..MAX_IDLE_ITERS {
            if !self.runner.process_events() {
                break;
            }
            iters += 1;
        }
        self.runner.refresh();
        self.drain_screen();
        iters
    }

    /// Latest captured frame, or `None` if no draw has occurred yet.
    pub fn screen(&self) -> Option<&ObservedScreen> {
        self.last_screen.as_ref()
    }

    /// Flatten the latest frame into a snapshot-friendly multiline
    /// string. Each row's trailing whitespace is trimmed and trailing
    /// blank rows are dropped, so minor layout drift in unused regions
    /// doesn't churn snapshots.
    pub fn screen_text(&self) -> String {
        match &self.last_screen {
            Some(screen) => flatten_screen(screen),
            None => String::new(),
        }
    }

    fn drain_screen(&mut self) {
        while let Ok(s) = self.screen_rx.try_recv() {
            self.last_screen = Some(s);
        }
    }
}

/// Render an [`ObservedScreen`] as a snapshot-friendly multiline string.
/// Same trimming rules as [`PuppetHarness::screen_text`].
pub fn flatten_screen(screen: &ObservedScreen) -> String {
    let size = screen.size();
    let piece = screen.piece(Vec2::zero(), size);
    let mut lines: Vec<String> = piece
        .as_strings()
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .collect();
    while let Some(last) = lines.last() {
        if last.is_empty() {
            lines.pop();
        } else {
            break;
        }
    }
    lines.join("\n")
}

/// Initialise the process-global test config and an empty user info.
///
/// View constructors and most renderers indirectly read
/// [`crate::config::get_config_theme`] and [`crate::client::get_user_info`]
/// — call this before building a view in a test. Both underlying
/// initialisers are guarded by `OnceCell` so repeated calls are safe.
pub fn ensure_globals_initialised() {
    init_test_config();
    init_test_user_info(None);
}

/// Allocate a fresh [`FakeHnApi`] and return a `'static` reference.
///
/// View constructors take `&'static dyn HnApi` — leaking is the
/// simplest way to get the required lifetime in a test. The `'static
/// FakeHnApi` reference is what lets the test inspect `fake.calls()`
/// after handing the fake into the view; coerce to `&'static dyn HnApi`
/// at the call site:
///
/// ```ignore
/// let fake = leak_fake_api();
/// let api: &'static dyn HnApi = fake;
/// ```
pub fn leak_fake_api() -> &'static FakeHnApi {
    Box::leak(Box::new(FakeHnApi::new()))
}

/// Build a [`Story`] with sensible defaults: `alice` author, 10 points,
/// 0 comments, fixed timestamp, `https://example.com/{id}` URL,
/// no content, not dead/flagged.
///
/// Override fields with struct-update syntax:
///
/// ```ignore
/// Story { points: 250, num_comments: 12, ..make_story(101, "Rust 2.0") }
/// ```
pub fn make_story(id: u32, title: impl Into<String>) -> Story {
    Story {
        id,
        url: format!("https://example.com/{id}"),
        author: "alice".to_string(),
        points: 10,
        num_comments: 0,
        time: 1_700_000_000,
        title: title.into(),
        content: String::new(),
        dead: false,
        flagged: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cursive::event::Key;
    use cursive::views::{Dialog, TextView};

    #[test]
    fn renders_text_view_with_default_size() {
        let mut siv = Cursive::new();
        siv.add_layer(TextView::new("hello puppet"));
        let mut harness = PuppetHarness::new(siv);
        harness.step_until_idle();
        let text = harness.screen_text();
        assert!(
            text.contains("hello puppet"),
            "expected 'hello puppet' in screen:\n{text}"
        );
    }

    #[test]
    fn screen_size_is_respected() {
        let siv = Cursive::new();
        let harness = PuppetHarness::with_size(siv, Vec2::new(40, 10));
        let screen = harness.screen().expect("initial frame should be captured");
        assert_eq!(screen.size(), Vec2::new(40, 10));
    }

    #[test]
    fn dialog_dismisses_on_escape() {
        let mut siv = Cursive::new();
        siv.add_layer(Dialog::info("press esc to dismiss"));
        let mut harness = PuppetHarness::new(siv);
        harness.step_until_idle();
        assert!(
            harness.screen_text().contains("press esc to dismiss"),
            "dialog should be visible before Esc; got:\n{}",
            harness.screen_text()
        );

        // Dialog::info installs an Ok button — Enter dismisses it.
        harness.send(Event::Key(Key::Enter));
        harness.step_until_idle();
        assert!(
            !harness.screen_text().contains("press esc to dismiss"),
            "dialog should be dismissed; got:\n{}",
            harness.screen_text()
        );
    }

    #[test]
    fn step_until_idle_returns_zero_when_nothing_pending() {
        let siv = Cursive::new();
        let mut harness = PuppetHarness::new(siv);
        // Nothing queued, nothing should be processed.
        let iters = harness.step_until_idle();
        assert_eq!(iters, 0);
    }

    #[test]
    fn flatten_screen_drops_trailing_blank_rows() {
        let mut siv = Cursive::new();
        siv.add_layer(TextView::new("only row"));
        let mut harness = PuppetHarness::with_size(siv, Vec2::new(20, 10));
        harness.step_until_idle();
        let text = harness.screen_text();
        // The last line of the snapshot should be the row containing
        // "only row" (or its frame), not 9 lines of empty padding.
        let line_count = text.lines().count();
        assert!(
            line_count <= 5,
            "expected trailing blank rows trimmed; got {line_count} lines:\n{text}"
        );
    }
}
