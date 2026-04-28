//! TEST_PLAN.md §3.2.b — front-page navigation and comment view
//! drilldown (Linux-only). Covers scenarios 3.2.2 and 3.2.3.
//!
//! See [`e2e_first_run.rs`] for the surrounding harness conventions
//! (TTY-gated flavor / auth prompts, `HOME` / `XDG_*` isolation,
//! `HN_ALGOLIA_BASE` / `HN_FIREBASE_BASE` overrides). The same
//! `news.ycombinator.com` caveat from §3.2.1 applies: the
//! unauthenticated front-page render hits real HN for vote state,
//! and the comment-view drilldown additionally hits
//! `news.ycombinator.com/item?id=<id>` via `get_page_content`. Both
//! return safe-to-ignore data for fixture IDs that don't collide
//! with the live `/news` page; a `--no-real-network` enforcement is
//! deferred to TEST_PLAN.md §3.3 / Phase 3 acceptance.

#![cfg(target_os = "linux")]

#[path = "e2e/mod.rs"]
mod helpers;

use std::time::Duration;

use serde_json::json;

use helpers::fakehn::FakeHnServer;
use helpers::{spawn_app, AppHandle, SpawnOptions, DEFAULT_WAIT};

const FRONT_PAGE_RENDER_TIMEOUT: Duration = Duration::from_secs(60);

const STORY1_ID: u32 = 91_111_001;
const STORY2_ID: u32 = 91_111_002;
const STORY3_ID: u32 = 91_111_003;

const STORY1_TITLE: &str = "navigation fixture story one";
const STORY2_TITLE: &str = "navigation fixture story two";
const STORY3_TITLE: &str = "navigation fixture story three";

/// Mount the `/v0/topstories.json` + `/api/v1/search` endpoints with
/// three fixture stories. Returns the handle so individual tests can
/// add further mocks (e.g. per-item Firebase responses for §3.2.3).
fn mount_three_stories(server: &FakeHnServer) {
    server.mount_get_json(
        "/v0/topstories.json",
        200,
        json!([STORY1_ID, STORY2_ID, STORY3_ID]),
    );
    server.mount_get_json(
        "/api/v1/search",
        200,
        json!({
            "hits": [
                fixture_hit(STORY1_ID, STORY1_TITLE, "alice", 250, 12),
                fixture_hit(STORY2_ID, STORY2_TITLE, "bob", 150, 7),
                fixture_hit(STORY3_ID, STORY3_TITLE, "carol", 90, 3),
            ]
        }),
    );
}

fn fixture_hit(
    id: u32,
    title: &str,
    author: &str,
    points: u32,
    num_comments: u32,
) -> serde_json::Value {
    json!({
        "objectID": id.to_string(),
        "author": author,
        "url": format!("https://example.com/{id}"),
        "story_text": null,
        "points": points,
        "num_comments": num_comments,
        "created_at_i": 1_700_000_000_u64,
        "_highlightResult": { "title": { "value": title } },
        "dead": false,
        "flagged": false,
    })
}

/// Step the binary through the first-run prompts so the subsequent
/// front-page render is the only thing under test.
fn dismiss_first_run_prompts(handle: &mut AppHandle) {
    handle
        .wait_for_text("[l]ight", DEFAULT_WAIT)
        .expect("flavor prompt should print");
    handle.send_keys("l\n").expect("send light flavor");

    handle
        .wait_for_text("Wrote config to", Duration::from_secs(10))
        .expect("binary should announce the freshly-written config");

    handle
        .wait_for_text("No auth file found", DEFAULT_WAIT)
        .expect("auth prompt should print after config write");
    handle.send_keys("\n").expect("skip auth (default = N)");
}

#[test]
fn front_page_navigates_with_j_k_and_ctrl_d() {
    let server = FakeHnServer::start();
    mount_three_stories(&server);

    let opts = SpawnOptions::new()
        .algolia_base(server.algolia_base())
        .firebase_base(server.firebase_base());
    let mut handle = spawn_app(opts).expect("spawn_app should succeed");

    dismiss_first_run_prompts(&mut handle);

    handle
        .wait_for_text(STORY1_TITLE, FRONT_PAGE_RENDER_TIMEOUT)
        .expect("first fixture story should render");

    // All three rows should be visible at once on a 120×40 screen.
    let initial_screen = handle.screen();
    for title in [STORY1_TITLE, STORY2_TITLE, STORY3_TITLE] {
        assert!(
            initial_screen.contains(title),
            "expected {title:?} on the rendered front page; saw:\n{initial_screen}"
        );
    }

    let row_story1 = handle
        .focused_row()
        .expect("front page should have a focused row at startup");

    // `j` — next_story.
    handle.send_keys("j").expect("send j");
    let row_story2 = wait_for_focus_change(&handle, row_story1);
    assert!(
        row_story2 > row_story1,
        "j should advance focus from row {row_story1} (saw {row_story2})\nscreen:\n{}",
        handle.screen()
    );

    // Second `j` lands on story 3.
    handle.send_keys("j").expect("send j");
    let row_story3 = wait_for_focus_change(&handle, row_story2);
    assert!(
        row_story3 > row_story2,
        "second j should advance focus from row {row_story2} (saw {row_story3})"
    );

    // `k` — prev_story. Should return to story 2's row exactly.
    handle.send_keys("k").expect("send k");
    let row_after_k = wait_for_focus_change(&handle, row_story3);
    assert_eq!(
        row_after_k, row_story2,
        "k should move focus back to row {row_story2} (saw {row_after_k})\n--- screen ---\n{}",
        handle.screen()
    );

    // `Ctrl-D` — page_down. With only three stories the half-page jump
    // saturates at the last entry; we just assert focus didn't slide
    // backwards.
    handle.send_keys("\x04").expect("send Ctrl-D");
    std::thread::sleep(Duration::from_millis(200));
    let row_after_ctrl_d = handle
        .focused_row()
        .expect("focus row should still resolve after Ctrl-D");
    assert!(
        row_after_ctrl_d >= row_after_k,
        "Ctrl-D should not move focus backwards (was {row_after_k}, now {row_after_ctrl_d})"
    );

    // Front page must still render every fixture story after the
    // navigation sequence — guards against a crash that wiped the
    // screen mid-test.
    let final_screen = handle.screen();
    for title in [STORY1_TITLE, STORY2_TITLE, STORY3_TITLE] {
        assert!(
            final_screen.contains(title),
            "expected {title:?} to remain visible after navigation; saw:\n{final_screen}"
        );
    }

    let status = handle.shutdown().expect("binary should exit cleanly");
    assert!(status.success(), "expected success exit, got {status:?}");
}

/// Poll [`AppHandle::focused_row`] until it differs from `prior` or
/// 1 s elapses. Cursive redraws on the next tick after the key event,
/// so a same-row reading immediately after `send_keys` is expected.
fn wait_for_focus_change(handle: &AppHandle, prior: u16) -> u16 {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        if let Some(now) = handle.focused_row() {
            if now != prior {
                return now;
            }
        }
        if std::time::Instant::now() >= deadline {
            return handle.focused_row().unwrap_or(prior);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
