# Hackernews-TIM Improvements over upstream hackernews-TUI

This fork has diverged from upstream `hackernews-TUI`. The sections below summarize every change authored by Benjamin Porter. None have been upstreamed.

## ⚠️ Breaking / behavior changes

Please review these before merging upstream. Most ship with automatic migration, but user-visible defaults, paths, and names have shifted.

- **Project renamed** from `hackernews-TUI` → `Hackernews-TIM`. Binary/crate is `hackernews_tim`.
- **Config file renamed** `hn-tui.toml` → `config.toml`. Legacy name is auto-migrated in place on first run.
- **Config/auth files relocated** from `$XDG_CONFIG_HOME/` (and `$HOME/.config/`) into a `hackernews-tim/` subdirectory. Legacy files are copied on first run (originals preserved).
- **Log file** moved from the config dir into the `hackernews-tim` cache subdirectory.
- **Default dark theme** selection colors were subdued — users with the shipped dark theme will see a different highlight color.
- **New default keybindings** were added (see below). Existing bindings are unchanged, but arrow keys, `Ctrl+u`/`Ctrl+d`, `/`, `n`, `N` are now claimed by the app in list/article views.
- `toml` dependency bumped to 1.1.0; `config_parser` tests updated for the 1.x `FromStr` behavior change.

## New features

### Authentication & identity
- Interactive HN login with first-run prompt and in-app login dialog.
- Session cookie is cached in the auth file to avoid re-logging in every startup; auth files are upgraded in place.
- Startup login outcome is reported to the user; bad credentials are no longer silently saved (hardened success detection).
- Logged-in **username and karma** shown in view title bars.
- User's HN profile **topcolor** is applied to the title bar.
- Authenticated user's own stories/comments marked with an orange `*`.
- "Open your own HN comments" global keybind.
- HN `showdead` profile setting is honored when authenticated.

### Voting
- **Upvote/downvote stories** from the story list.
- **Downvote comments** (up arrow already existed); down arrow now renders in the comment view.
- Up-arrow indicator shown next to voteable stories in the story view.
- Vote state is pre-fetched for story lists so arrows render on every open and on every page.
- Past votes are read from HN's post-vote markup.
- **Vouch** for dead stories and comments.

### Comments & replies
- Comments loaded from the HN web page when authenticated (surfaces dead/flagged content authenticated users can see).
- Reply-via-editor flow in the comment view; reply keybind in the story view; reply promoted into `CommentViewKeyMap`.
- Edit-your-own-comment flow on the comment view.
- `[dead]` and `[flagged]` badges prefix the byline of such items; their bodies are faded.

### Find-on-page
- New `/` find-on-page feature across comment, story, article, and search views.
- `n` / `N` jump forward/back through matches; paging handlers are gated so these keys don't collide.
- `Esc` exits find mode outside the dialog.

### Navigation
- Arrow keys bound alongside `h/j/k/l` in story and comment views.
- `Ctrl+u` / `Ctrl+d` for half-page scrolling; `PageUp`/`PageDown` now move focus by half a page in list views.

### Config
- `--init-config <light|dark>` flag writes a default config and exits; first-run prompt offers the same.
- `--update-theme <light|dark>` flag replaces just the `[theme]` table of an existing config in place (mutually exclusive with `--init-config`).
- **Story listing page size** is configurable.
- **Search-view page size** is configurable.

## Build, tooling, packaging

- Added top-level `Makefile` with build, install (honors `PREFIX`), lint, clippy, format, test, docker, and cross-compile targets; `make help` auto-generates from `##` comments.
- Added `hackernews_tui(1)` man page; installed by `make install`.
- Added `CLAUDE.md` with build commands and architecture overview.
- Silenced all compiler and clippy warnings across the workspace; normalized view files with `rustfmt`.
- Test fixture CSRF tokens replaced with synthetic values.
- Snapshot test added for `parse_reply_form` against a real HN response.

## Commit count

59 commits by Benjamin Porter on `main`, all dated 2026-04-23 to 2026-04-24.
