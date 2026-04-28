# tests/fixtures

Raw network responses (HTML / JSON) used by tests, captured from real
upstream sources (`news.ycombinator.com`, `hn.algolia.com`,
`hacker-news.firebaseio.com`) and committed verbatim. They are read
directly from disk by tests — no `include_str!` — so adding or editing
a fixture does not require recompiling.

## Loading a fixture

Use the helpers in `crate::test_support::fixtures`:

```rust
use hackernews_tim::test_support::fixtures::{fixture_path, read_fixture};

let html = read_fixture("comment_page.html");
// or, when an API takes &Path:
let path = fixture_path("reply_form.html");
```

Both resolve through `CARGO_MANIFEST_DIR`, so they work regardless of
the current working directory the test process happens to be launched
from.

## When to add a fixture file

Add an HTML/JSON file when:

- The shape of an upstream response is what's under test (HN scraping,
  reply-form parsing, profile parsing, etc.).
- The fixture is large enough that inlining it as a `r#"..."#` literal
  would dominate the test source.
- More than one test references the same captured response.

Keep fixtures **inline** (as plain Rust struct literals) when the data
is small, intrinsically tied to one test, and easier to read at the
call site than to look up in a separate file. Most view-level tests
construct `Story` / `Comment` / `Article` values inline through the
`crate::test_support::make_story` helper plus struct-update syntax —
that is the recommended pattern for view tests.

## `FakeHnApi` configuration

`FakeHnApi` is configured per-test by handing it the response a given
call should return. There is no global / shared fixture state; each
test allocates a fresh fake via `crate::test_support::leak_fake_api()`
and registers exactly the responses it cares about:

```rust
use hackernews_tim::test_support::{leak_fake_api, make_story};

let fake = leak_fake_api();
fake.set_stories_for_tag("front_page", vec![
    make_story(101, "First"),
    make_story(102, "Second"),
]);
let api: &'static dyn HnApi = fake;
// pass `api` into the view; inspect `fake.calls()` after.
```

Methods that aren't configured return sensible empty values rather than
erroring, so a test that only cares about a single endpoint doesn't
have to scaffold the rest of the API surface.
