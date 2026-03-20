use std::cmp::Reverse;
use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local};
use clap::Parser;
use dialoguer::{Select, theme::ColorfulTheme};
use serde_json::Value;

const SESSION_LIMIT: usize = 10;
const BASH_STDOUT_PREFIX: &str = "<bash-stdout>";
const BASH_INPUT_PREFIX: &str = "<bash-input>";
const BASH_INPUT_SUFFIX: &str = "</bash-input>";
const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BOLD_BLUE: &str = "\x1b[1;34m";
const ANSI_BOLD_GREEN: &str = "\x1b[1;32m";
const ANSI_BOLD_YELLOW: &str = "\x1b[1;33m";
const ANSI_DIM: &str = "\x1b[2m";

#[derive(Debug, Parser)]
#[command(about = "Get the history of conversations with Claude Code.")]
struct Cli {
    #[arg(short = 'b', long = "include-bash-output")]
    include_bash_output: bool,

    #[arg(short = 'l', long = "list")]
    list: bool,

    #[arg(short = 's', long = "choose-session")]
    choose_session: bool,
}

#[derive(Debug, Clone)]
struct SessionSummary {
    path: PathBuf,
    modified_at: DateTime<Local>,
    first_prompt: String,
}

fn main() {
    if let Err(err) = run() {
        let _ = writeln!(io::stderr(), "{err}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().context("failed to determine current directory")?;
    let project_dir = claude_project_dir(&cwd)?;

    if !project_dir.exists() {
        bail!("No Claude history found for this folder");
    }

    let mut sessions = collect_sessions(&project_dir)?;
    if sessions.is_empty() {
        bail!("No session files found in the Claude project directory.");
    }

    if cli.list {
        return print_session_list(&sessions);
    }

    let session = if cli.choose_session {
        choose_session(&sessions)?
    } else {
        sessions.remove(0)
    };

    print_session(&session.path, cli.include_bash_output)
}

fn claude_project_dir(cwd: &Path) -> Result<PathBuf> {
    let project_id = cwd.to_string_lossy().replace('/', "-");
    let home = std::env::var("HOME").context("HOME is not set")?;
    Ok(Path::new(&home)
        .join(".claude")
        .join("projects")
        .join(project_id))
}

fn collect_sessions(project_dir: &Path) -> Result<Vec<SessionSummary>> {
    let mut sessions = Vec::new();

    for entry in project_dir
        .read_dir()
        .with_context(|| format!("failed to read {}", project_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }

        sessions.push(session_summary(&path)?);
    }

    sessions.sort_by_key(|session| Reverse(session.modified_at));
    sessions.truncate(SESSION_LIMIT);
    Ok(sessions)
}

fn session_summary(path: &Path) -> Result<SessionSummary> {
    let metadata = path
        .metadata()
        .with_context(|| format!("failed to read metadata for {}", path.display()))?;
    let modified_at: DateTime<Local> = DateTime::from(
        metadata
            .modified()
            .with_context(|| format!("failed to read mtime for {}", path.display()))?,
    );

    let reader = open_jsonl(path)?;
    let mut first_prompt = String::from("Unknown");

    for line in reader.lines() {
        let line = line.with_context(|| format!("failed to read {}", path.display()))?;
        let data: Value = serde_json::from_str(&line)
            .with_context(|| format!("failed to parse JSON in {}", path.display()))?;

        if is_message(&data)
            && let Some(content) = message_content(&data)
        {
            first_prompt = collapse_newlines(content.trim());
            break;
        }
    }

    Ok(SessionSummary {
        path: path.to_path_buf(),
        modified_at,
        first_prompt,
    })
}

fn choose_session(sessions: &[SessionSummary]) -> Result<SessionSummary> {
    let entries = sessions
        .iter()
        .map(|session| {
            format!(
                "({}) {}",
                session.modified_at.format("%Y-%m-%d %H:%M:%S"),
                session.first_prompt
            )
        })
        .collect::<Vec<_>>();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a session")
        .items(&entries)
        .default(0)
        .interact_opt()
        .context("failed to read terminal selection")?;

    match selection {
        Some(index) => Ok(sessions[index].clone()),
        None => process::exit(0),
    }
}

fn print_session_list(sessions: &[SessionSummary]) -> Result<()> {
    let use_color = stdout_supports_color();
    for session in sessions {
        let timestamp = session.modified_at.format("%Y-%m-%d %H:%M:%S");
        if use_color {
            println!(
                "{ANSI_DIM}{timestamp}{ANSI_RESET}  {}",
                session.first_prompt
            );
        } else {
            println!("{timestamp}  {}", session.first_prompt);
        }
    }
    Ok(())
}

fn print_session(path: &Path, include_bash_output: bool) -> Result<()> {
    let reader = open_jsonl(path)?;
    let use_color = stdout_supports_color();

    for line in reader.lines() {
        let line = line.with_context(|| format!("failed to read {}", path.display()))?;
        let data: Value = serde_json::from_str(&line)
            .with_context(|| format!("failed to parse JSON in {}", path.display()))?;

        if !is_message(&data) {
            continue;
        }

        let Some(content) = message_content(&data) else {
            continue;
        };

        let mut content = content.trim().to_owned();
        if content.starts_with(BASH_STDOUT_PREFIX) && !include_bash_output {
            continue;
        }

        if let Some(inner) = content
            .strip_prefix(BASH_INPUT_PREFIX)
            .and_then(|value| value.strip_suffix(BASH_INPUT_SUFFIX))
        {
            content = format!("! {}", inner.trim());
        }

        let role = data
            .get("message")
            .and_then(|message| message.get("role"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let timestamp = data
            .get("timestamp")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        println!("{}", format_speaker_heading(role, timestamp, use_color));
        println!();

        if role == "user" {
            println!("{}", quote_markdown(&content, use_color));
        } else {
            println!("{content}");
        }

        println!();
    }

    Ok(())
}

fn open_jsonl(path: &Path) -> Result<BufReader<File>> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    Ok(BufReader::new(file))
}

fn message_content(data: &Value) -> Option<&str> {
    let content = data.get("message")?.get("content")?;

    if let Some(text) = content.as_str() {
        return Some(text);
    }

    content
        .as_array()?
        .iter()
        .find_map(|item| item.as_object()?.get("text").and_then(Value::as_str))
}

fn is_message(data: &Value) -> bool {
    !data
        .get("isSidechain")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !data.get("isMeta").and_then(Value::as_bool).unwrap_or(false)
        && data.get("toolUseResult").is_none()
        && message_content(data).is_some()
}

fn collapse_newlines(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\n' {
            result.push('\\');
            result.push('n');

            while matches!(chars.peek(), Some(next) if next.is_whitespace()) {
                chars.next();
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn stdout_supports_color() -> bool {
    io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").unwrap_or_default() != "dumb"
}

fn format_speaker_heading(role: &str, timestamp: &str, use_color: bool) -> String {
    let label = match role {
        "user" => "User",
        "assistant" => "Assistant",
        _ => role,
    };
    let heading = format!("**{label}**");
    let heading = if use_color {
        colorize_heading(role, &heading)
    } else {
        heading
    };

    match format_timestamp(timestamp) {
        Some(time) if use_color => {
            format!("{heading} {ANSI_DIM}_({time})_{ANSI_RESET}")
        }
        Some(time) => format!("{heading} _({time})_"),
        None => heading,
    }
}

fn format_timestamp(timestamp: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|parsed| parsed.with_timezone(&Local).format("%H:%M").to_string())
}

fn colorize_heading(role: &str, heading: &str) -> String {
    let color = match role {
        "user" => ANSI_BOLD_BLUE,
        "assistant" => ANSI_BOLD_GREEN,
        _ => ANSI_BOLD_YELLOW,
    };

    format!("{color}{heading}{ANSI_RESET}")
}

fn quote_markdown(text: &str, use_color: bool) -> String {
    let mut quoted = String::new();
    let quote_prefix = if use_color {
        format!("{ANSI_DIM}> {ANSI_RESET}")
    } else {
        String::from("> ")
    };
    let blank_quote_prefix = if use_color {
        format!("{ANSI_DIM}>{ANSI_RESET}")
    } else {
        String::from(">")
    };

    for (index, line) in text.lines().enumerate() {
        if index > 0 {
            quoted.push('\n');
        }

        if line.is_empty() {
            quoted.push_str(&blank_quote_prefix);
        } else {
            quoted.push_str(&quote_prefix);
            quoted.push_str(line);
        }
    }

    if quoted.is_empty() {
        quoted.push_str(&blank_quote_prefix);
    }

    quoted
}

#[cfg(test)]
mod tests {
    use super::{
        ANSI_BOLD_BLUE, ANSI_DIM, ANSI_RESET, collapse_newlines, format_speaker_heading,
        format_timestamp, is_message, message_content, quote_markdown,
    };
    use serde_json::json;

    #[test]
    fn extracts_string_message_content() {
        let value = json!({
            "message": {
                "content": "hello"
            }
        });

        assert_eq!(message_content(&value), Some("hello"));
    }

    #[test]
    fn extracts_first_text_item_from_array_content() {
        let value = json!({
            "message": {
                "content": [
                    {"text": "hello"},
                    {"text": "ignored"}
                ]
            }
        });

        assert_eq!(message_content(&value), Some("hello"));
    }

    #[test]
    fn extracts_text_when_array_starts_with_non_text_item() {
        let value = json!({
            "message": {
                "content": [
                    {"type": "tool_use", "name": "shell"},
                    {"text": "hello"}
                ]
            }
        });

        assert_eq!(message_content(&value), Some("hello"));
    }

    #[test]
    fn filters_out_tool_results_and_sidechains() {
        let message = json!({
            "isSidechain": false,
            "message": {
                "content": "hello"
            }
        });
        let sidechain = json!({
            "isSidechain": true,
            "message": {
                "content": "hello"
            }
        });
        let tool_result = json!({
            "message": {
                "content": "hello"
            },
            "toolUseResult": {"stdout": "nope"}
        });

        assert!(is_message(&message));
        assert!(!is_message(&sidechain));
        assert!(!is_message(&tool_result));
    }

    #[test]
    fn collapses_newlines_like_python_version() {
        assert_eq!(collapse_newlines("a\n  b\n\nc"), r"a\nb\nc");
    }

    #[test]
    fn formats_rfc3339_timestamp_as_compact_time() {
        assert_eq!(
            format_timestamp("2026-03-19T10:23:45+00:00"),
            Some("10:23".to_string())
        );
    }

    #[test]
    fn omits_timestamp_when_it_cannot_be_parsed() {
        assert_eq!(format_speaker_heading("user", "unknown", false), "**User**");
    }

    #[test]
    fn quotes_each_user_line_for_markdown() {
        assert_eq!(
            quote_markdown("first\n\nsecond", false),
            "> first\n>\n> second"
        );
    }

    #[test]
    fn colors_speaker_heading_for_tty_output() {
        assert_eq!(
            format_speaker_heading("user", "2026-03-19T10:23:45+00:00", true),
            format!("{ANSI_BOLD_BLUE}**User**{ANSI_RESET} {ANSI_DIM}_(10:23)_{ANSI_RESET}")
        );
    }

    #[test]
    fn colors_quote_markers_for_tty_output() {
        assert_eq!(
            quote_markdown("first\n", true),
            format!("{ANSI_DIM}> {ANSI_RESET}first")
        );
    }
}
