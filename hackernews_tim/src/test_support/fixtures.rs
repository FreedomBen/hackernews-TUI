//! File-based fixture loading for integration tests.
//!
//! Fixtures live under `hackernews_tim/tests/fixtures/` (raw HTML or
//! JSON captured from real upstream responses). [`fixture_path`] resolves
//! a fixture by name and [`read_fixture`] reads it into a `String`. Both
//! resolve the directory via `CARGO_MANIFEST_DIR` so they work no matter
//! the current working directory.
//!
//! See `tests/fixtures/README.md` for the discipline this module
//! supports.

use std::path::{Path, PathBuf};

/// Absolute path to `hackernews_tim/tests/fixtures/`.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Resolve a fixture path by file name, e.g. `fixture_path("comment_page.html")`.
pub fn fixture_path(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

/// Read a fixture file into a `String`. Panics on I/O error so the
/// test fails immediately and points at the missing path.
pub fn read_fixture(name: &str) -> String {
    let path = fixture_path(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_dir_resolves_to_tests_fixtures() {
        let dir = fixtures_dir();
        assert!(dir.ends_with("tests/fixtures"), "got: {}", dir.display());
        assert!(dir.is_dir(), "fixtures dir should exist: {}", dir.display());
    }

    #[test]
    fn read_fixture_loads_known_html() {
        let html = read_fixture("comment_page.html");
        assert!(!html.is_empty(), "comment_page.html should not be empty");
    }

    #[test]
    #[should_panic(expected = "read fixture")]
    fn read_fixture_panics_on_missing_file() {
        let _ = read_fixture("definitely_does_not_exist.html");
    }
}
