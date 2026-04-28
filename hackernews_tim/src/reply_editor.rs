use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

// Git-style "below this line is ignored" marker. Splits the temp file
// into the reply body (above) and the quoted parent (below).
const SCISSORS: &str = "# ------ >8 ------";

pub enum PendingAction {
    ReplyTo {
        parent_id: u32,
        parent_content: String,
        // Root of the comment view the reply was invoked from, so the
        // TUI re-opens on the same thread the user was reading instead
        // of rooting on the individual comment they replied to.
        return_to_id: u32,
    },
    EditComment {
        comment_id: u32,
        return_to_id: u32,
    },
}

pub fn run_editor_for_reply(parent: &str) -> Result<Option<String>> {
    let editor = std::env::var("VISUAL")
        .ok()
        .or_else(|| std::env::var("EDITOR").ok())
        .unwrap_or_else(|| "vi".to_string());

    let path = scratch_path();
    write_scaffold(&path, parent)
        .with_context(|| format!("writing scaffold to {}", path.display()))?;

    let status =
        run_editor(&editor, &path).with_context(|| format!("spawning editor `{editor}`"))?;
    if !status.success() {
        let _ = fs::remove_file(&path);
        anyhow::bail!("editor `{editor}` exited with status {status}");
    }

    let body = read_and_strip(&path).with_context(|| format!("reading {}", path.display()))?;
    let _ = fs::remove_file(&path);

    if body.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(body))
    }
}

fn run_editor(editor: &str, file: &Path) -> std::io::Result<std::process::ExitStatus> {
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let mut cmd = Command::new(program);
    for arg in parts {
        cmd.arg(arg);
    }
    cmd.arg(file).status()
}

fn scratch_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    p.push(format!("hn-reply-{pid}-{nanos}.md"));
    p
}

fn write_scaffold(path: &Path, parent: &str) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f)?;
    writeln!(f, "{SCISSORS}")?;
    writeln!(f, "# Write your reply above the scissors line.")?;
    writeln!(f, "# Save and exit to submit; leave it blank to abort.")?;
    writeln!(f, "#")?;
    writeln!(f, "# Replying to:")?;
    writeln!(f, "#")?;
    let mut any = false;
    for line in parent.lines() {
        writeln!(f, "# > {line}")?;
        any = true;
    }
    if !any {
        writeln!(f, "# > (empty)")?;
    }
    Ok(())
}

pub fn run_editor_for_edit(current_text: &str) -> Result<Option<String>> {
    let editor = std::env::var("VISUAL")
        .ok()
        .or_else(|| std::env::var("EDITOR").ok())
        .unwrap_or_else(|| "vi".to_string());

    let path = scratch_path();
    write_edit_scaffold(&path, current_text)
        .with_context(|| format!("writing scaffold to {}", path.display()))?;

    let status =
        run_editor(&editor, &path).with_context(|| format!("spawning editor `{editor}`"))?;
    if !status.success() {
        let _ = fs::remove_file(&path);
        anyhow::bail!("editor `{editor}` exited with status {status}");
    }

    let body = read_and_strip(&path).with_context(|| format!("reading {}", path.display()))?;
    let _ = fs::remove_file(&path);

    if body.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(body))
    }
}

fn write_edit_scaffold(path: &Path, current_text: &str) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "{current_text}")?;
    writeln!(f, "{SCISSORS}")?;
    writeln!(f, "# Edit your comment above the scissors line.")?;
    writeln!(f, "# Save unchanged to cancel; clear the body to cancel.")?;
    Ok(())
}

pub fn wait_for_enter() {
    eprintln!("Press Enter to return to the TUI...");
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
}

fn read_and_strip(path: &Path) -> std::io::Result<String> {
    let mut s = String::new();
    fs::File::open(path)?.read_to_string(&mut s)?;
    let body: String = s
        .lines()
        .take_while(|l| l.trim_end() != SCISSORS)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(body.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(suffix: &str) -> PathBuf {
        // Match the pattern existing test code uses (e.g. config::init tests).
        std::env::temp_dir().join(format!(
            "hn-reply-editor-test-{}-{}-{}.md",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0),
            suffix
        ))
    }

    struct TempFile(PathBuf);
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).expect("read")
    }

    #[test]
    fn write_scaffold_quotes_parent_with_hash_arrow_prefix() {
        let path = TempFile(temp_path("write_scaffold_quotes"));
        write_scaffold(&path.0, "first line\nsecond line").expect("write");
        let body = read(&path.0);
        assert!(body.contains(SCISSORS), "scissors missing: {body:?}");
        // Parent is quoted with `# > ` (entire block is also a comment).
        assert!(
            body.contains("# > first line"),
            "expected '# > first line' in {body:?}"
        );
        assert!(
            body.contains("# > second line"),
            "expected '# > second line' in {body:?}"
        );
        // Instructional comments accompany the scissors line.
        assert!(body.contains("Write your reply above the scissors line"));
    }

    #[test]
    fn write_scaffold_handles_empty_parent() {
        let path = TempFile(temp_path("write_scaffold_empty"));
        write_scaffold(&path.0, "").expect("write");
        let body = read(&path.0);
        // The empty-parent fallback inserts a sentinel.
        assert!(
            body.contains("# > (empty)"),
            "expected empty placeholder; got {body:?}"
        );
    }

    #[test]
    fn write_edit_scaffold_includes_current_text_and_scissors() {
        let path = TempFile(temp_path("write_edit_scaffold"));
        write_edit_scaffold(&path.0, "current comment text").expect("write");
        let body = read(&path.0);
        assert!(body.contains("current comment text"), "got {body:?}");
        assert!(body.contains(SCISSORS), "scissors missing: {body:?}");
        assert!(body.contains("Edit your comment above the scissors line"));
    }

    #[test]
    fn read_and_strip_returns_body_above_scissors() {
        let path = TempFile(temp_path("read_and_strip_body"));
        fs::write(
            &path.0,
            format!("hello world\nsecond line\n{SCISSORS}\n# instructions\n# > parent\n"),
        )
        .expect("write");
        let body = read_and_strip(&path.0).expect("read_and_strip");
        assert_eq!(body, "hello world\nsecond line");
    }

    #[test]
    fn read_and_strip_trims_surrounding_whitespace() {
        let path = TempFile(temp_path("read_and_strip_trim"));
        fs::write(&path.0, format!("\n  body  \n\n{SCISSORS}\n# rest\n")).expect("write");
        let body = read_and_strip(&path.0).expect("read_and_strip");
        assert_eq!(body, "body");
    }

    #[test]
    fn read_and_strip_returns_empty_when_only_scaffold_remains() {
        let path = TempFile(temp_path("read_and_strip_empty"));
        // write_scaffold output only (parent text below the scissors).
        write_scaffold(&path.0, "some parent text").expect("write");
        let body = read_and_strip(&path.0).expect("read_and_strip");
        assert_eq!(body, "");
    }

    #[test]
    fn scratch_path_lives_in_system_temp_dir() {
        let p = scratch_path();
        assert!(
            p.starts_with(std::env::temp_dir()),
            "{p:?} not under {:?}",
            std::env::temp_dir()
        );
        let name = p.file_name().unwrap().to_str().unwrap();
        let pid = std::process::id().to_string();
        assert!(
            name.starts_with(&format!("hn-reply-{pid}-")),
            "unexpected file name: {name:?}"
        );
        assert!(name.ends_with(".md"), "unexpected extension in {name:?}");
    }

    #[test]
    fn scratch_path_consecutive_calls_yield_distinct_paths() {
        let a = scratch_path();
        // Different nanos guarantee a different filename even for the same pid.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = scratch_path();
        assert_ne!(a, b);
    }
}
