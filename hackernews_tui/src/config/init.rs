use std::io::{self, IsTerminal, Write};
use std::path::Path;

const LIGHT_CONFIG: &str = include_str!("../../../examples/hn-tui.toml");
const DARK_CONFIG: &str = include_str!("../../../examples/hn-tui-dark.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFlavor {
    Light,
    Dark,
}

impl ConfigFlavor {
    pub fn contents(self) -> &'static str {
        match self {
            ConfigFlavor::Light => LIGHT_CONFIG,
            ConfigFlavor::Dark => DARK_CONFIG,
        }
    }
}

impl std::str::FromStr for ConfigFlavor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "light" => Ok(ConfigFlavor::Light),
            "dark" => Ok(ConfigFlavor::Dark),
            other => Err(format!(
                "unknown config flavor '{other}' (expected 'light' or 'dark')"
            )),
        }
    }
}

pub fn write_default_config(path: &Path, flavor: ConfigFlavor) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, flavor.contents())?;
    Ok(())
}

/// Outcome of [`prompt_for_auth`].
pub enum AuthPromptResult {
    /// The user declined to log in. No file should be written.
    Skip,
    /// The user entered credentials — still need to verify + write.
    Credentials { username: String, password: String },
}

/// Interactively ask the user whether to log in to Hacker News, and if so
/// collect a username + password (password input is masked).
///
/// Returns `None` when stdin/stdout is not a TTY or the interaction fails,
/// so the caller can silently fall back to "no auth".
pub fn prompt_for_auth() -> Option<AuthPromptResult> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return None;
    }

    loop {
        print!("No auth file found. Log in to Hacker News now? [y/N]: ");
        if io::stdout().flush().is_err() {
            return None;
        }

        let mut buf = String::new();
        if io::stdin().read_line(&mut buf).is_err() {
            return None;
        }

        match buf.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => break,
            "n" | "no" | "" => return Some(AuthPromptResult::Skip),
            _ => eprintln!("Please enter 'y' or 'n'."),
        }
    }

    print!("Username: ");
    io::stdout().flush().ok()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username).ok()?;
    let username = username.trim().to_string();
    if username.is_empty() {
        eprintln!("Empty username, skipping login.");
        return Some(AuthPromptResult::Skip);
    }

    let password = match rpassword::prompt_password("Password: ") {
        Ok(p) => p,
        Err(err) => {
            eprintln!("Failed to read password: {err}. Skipping login.");
            return Some(AuthPromptResult::Skip);
        }
    };
    if password.is_empty() {
        eprintln!("Empty password, skipping login.");
        return Some(AuthPromptResult::Skip);
    }

    Some(AuthPromptResult::Credentials { username, password })
}

/// Interactively ask the user which default config flavor to write.
///
/// Returns `None` if either stdin or stdout is not a TTY, or if the user skips.
pub fn prompt_for_flavor() -> Option<ConfigFlavor> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return None;
    }

    loop {
        print!("No config file found. Create one now? [l]ight / [d]ark / [s]kip: ");
        if io::stdout().flush().is_err() {
            return None;
        }

        let mut buf = String::new();
        if io::stdin().read_line(&mut buf).is_err() {
            return None;
        }

        match buf.trim().to_ascii_lowercase().as_str() {
            "l" | "light" => return Some(ConfigFlavor::Light),
            "d" | "dark" => return Some(ConfigFlavor::Dark),
            "s" | "skip" | "" => return None,
            _ => eprintln!("Please enter 'l', 'd', or 's'."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flavor_from_str_accepts_both_cases() {
        assert_eq!(
            "light".parse::<ConfigFlavor>().unwrap(),
            ConfigFlavor::Light
        );
        assert_eq!(
            "LIGHT".parse::<ConfigFlavor>().unwrap(),
            ConfigFlavor::Light
        );
        assert_eq!("Dark".parse::<ConfigFlavor>().unwrap(), ConfigFlavor::Dark);
        assert!("midnight".parse::<ConfigFlavor>().is_err());
    }

    #[test]
    fn embedded_configs_are_nonempty_and_distinct() {
        assert!(!LIGHT_CONFIG.is_empty());
        assert!(!DARK_CONFIG.is_empty());
        assert_ne!(LIGHT_CONFIG, DARK_CONFIG);
    }

    #[test]
    fn write_default_config_creates_parent_dirs() {
        let tmp =
            std::env::temp_dir().join(format!("hackernews_tui_init_test_{}", std::process::id()));
        let path = tmp.join("nested").join("hn-tui.toml");
        let _ = std::fs::remove_dir_all(&tmp);

        write_default_config(&path, ConfigFlavor::Dark).expect("write should succeed");
        let contents = std::fs::read_to_string(&path).expect("file should exist");
        assert_eq!(contents, DARK_CONFIG);

        std::fs::remove_dir_all(&tmp).ok();
    }
}
