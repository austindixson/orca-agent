use std::io::{self, Write};
use std::time::Duration;

use crossterm::cursor;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{self, ClearType};
use serde_json::{json, Value};

use crate::config::OrcaConfig;
use crate::http_client::post_json;

const ORCA_LOGO: [&str; 5] = [
    "   ____  ____  _________ ",
    "  / __ \\/ __ \\/ ____/   |",
    " / / / / /_/ / /   / /| |",
    "/ /_/ / _, _/ /___/ ___ |",
    "\\____/_/ |_|\\____/_/  |_|",
];

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> anyhow::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            terminal::EnterAlternateScreen,
            terminal::Clear(ClearType::All),
            EnableMouseCapture,
            cursor::Hide
        )?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            cursor::Show,
            DisableMouseCapture,
            terminal::LeaveAlternateScreen,
            terminal::Clear(ClearType::All)
        );
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Clone)]
struct Line {
    role: &'static str,
    text: String,
}

fn best_reply_text(v: &Value) -> Option<String> {
    for key in ["reply", "text", "message", "output", "content"] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            if !s.trim().is_empty() {
                return Some(s.to_string());
            }
        }
    }
    if let Some(obj) = v.as_object() {
        for (_k, value) in obj {
            if let Some(found) = best_reply_text(value) {
                return Some(found);
            }
        }
    }
    if let Some(arr) = v.as_array() {
        for value in arr {
            if let Some(found) = best_reply_text(value) {
                return Some(found);
            }
        }
    }
    None
}

fn truncate_to(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if s.chars().count() <= width {
        return s.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let mut out = String::new();
    for ch in s.chars().take(width - 1) {
        out.push(ch);
    }
    out.push('…');
    out
}

fn pad_to(s: &str, width: usize) -> String {
    let t = truncate_to(s, width);
    let count = t.chars().count();
    if count >= width {
        return t;
    }
    format!("{}{}", t, " ".repeat(width - count))
}

fn wrap_text(s: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![];
    }

    let mut out = Vec::new();
    for raw_line in s.replace('\r', "").split('\n') {
        if raw_line.trim().is_empty() {
            out.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            if word.chars().count() > width {
                if !current.is_empty() {
                    out.push(current.clone());
                    current.clear();
                }
                let mut chunk = String::new();
                for ch in word.chars() {
                    chunk.push(ch);
                    if chunk.chars().count() == width {
                        out.push(chunk.clone());
                        chunk.clear();
                    }
                }
                if !chunk.is_empty() {
                    current = chunk;
                }
                continue;
            }

            if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + 1 + word.chars().count() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                out.push(current.clone());
                current.clear();
                current.push_str(word);
            }
        }

        if !current.is_empty() {
            out.push(current);
        }
    }

    if out.is_empty() {
        out.push(String::new());
    }

    out
}

fn transcript_rows(lines: &[Line], inner_width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let content_width = inner_width.saturating_sub(8).max(8);

    for line in lines {
        let tag = match line.role {
            "user" => "YOU",
            "assistant" => "ORCA",
            "system" => "SYS",
            _ => "LOG",
        };
        let wrapped = wrap_text(&line.text, content_width);
        for (idx, segment) in wrapped.iter().enumerate() {
            if idx == 0 {
                rows.push(format!("[{tag:<4}] {segment}"));
            } else {
                rows.push(format!("[    ] {segment}"));
            }
        }
    }

    rows
}

fn header_height() -> usize {
    ORCA_LOGO.len() + 4
}

fn footer_height() -> usize {
    4
}

fn transcript_height(total_height: usize) -> usize {
    total_height
        .saturating_sub(header_height() + footer_height())
        .max(1)
}

fn max_scroll(lines: &[Line], width: usize, height: usize) -> usize {
    let inner = width.saturating_sub(2);
    let rows = transcript_rows(lines, inner);
    rows.len().saturating_sub(transcript_height(height))
}

fn render(
    lines: &[Line],
    input: &str,
    status: &str,
    scroll: usize,
    endpoint: &str,
) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    let (w, h) = terminal::size()?;
    let width = w as usize;
    let height = h as usize;

    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::All)
    )?;

    if width < 48 || height < 16 {
        writeln!(stdout, "ORCA TUI")?;
        writeln!(stdout, "Terminal too small (need >=48x16).")?;
        writeln!(stdout, "Resize terminal. Esc quits.")?;
        stdout.flush()?;
        return Ok(());
    }

    let inner = width - 2;

    writeln!(stdout, "┏{}┓", "━".repeat(inner))?;
    for logo in ORCA_LOGO {
        writeln!(stdout, "┃{}┃", pad_to(logo, inner))?;
    }
    writeln!(
        stdout,
        "┃{}┃",
        pad_to(
            &format!(
                " endpoint: {}",
                truncate_to(endpoint, inner.saturating_sub(11))
            ),
            inner
        )
    )?;
    writeln!(
        stdout,
        "┃{}┃",
        pad_to(
            " controls: Enter send • ↑/↓ scroll • wheel scroll • Esc quit",
            inner
        )
    )?;
    writeln!(stdout, "┣{}┫", "━".repeat(inner))?;

    let rows = transcript_rows(lines, inner);
    let view_h = transcript_height(height);
    let start = scroll.min(rows.len().saturating_sub(view_h));
    let end = (start + view_h).min(rows.len());

    for row in &rows[start..end] {
        writeln!(stdout, "┃{}┃", pad_to(row, inner))?;
    }
    for _ in (end - start)..view_h {
        writeln!(stdout, "┃{}┃", " ".repeat(inner))?;
    }

    writeln!(stdout, "┣{}┫", "━".repeat(inner))?;
    writeln!(
        stdout,
        "┃{}┃",
        pad_to(
            &truncate_to(
                &format!(
                    " status: {} | messages: {} | scroll: {}",
                    status,
                    lines.len(),
                    start
                ),
                inner
            ),
            inner
        )
    )?;
    writeln!(
        stdout,
        "┃{}┃",
        pad_to(&truncate_to(&format!("> {}", input), inner), inner)
    )?;
    write!(stdout, "┗{}┛", "━".repeat(inner))?;
    stdout.flush()?;
    Ok(())
}

pub async fn run_tui() -> anyhow::Result<()> {
    let _guard = TerminalGuard::enter()?;
    let cfg = crate::merge_token(OrcaConfig::load()?);

    let mut lines = vec![Line {
        role: "system",
        text: "Connected. Type a prompt and press Enter.".to_string(),
    }];
    let mut input = String::new();
    let mut status = "ready".to_string();

    let (w0, h0) = terminal::size().unwrap_or((120, 40));
    let mut scroll: usize = max_scroll(&lines, w0 as usize, h0 as usize);

    loop {
        render(&lines, &input, &status, scroll, &cfg.base_url())?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Esc => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up => {
                        scroll = scroll.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        let (w, h) = terminal::size().unwrap_or((120, 40));
                        let max = max_scroll(&lines, w as usize, h as usize);
                        scroll = (scroll + 1).min(max);
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Enter => {
                        let text = input.trim().to_string();
                        input.clear();
                        if text.is_empty() {
                            continue;
                        }
                        lines.push(Line {
                            role: "user",
                            text: text.clone(),
                        });
                        status = "sending…".to_string();
                        render(&lines, &input, &status, scroll, &cfg.base_url())?;

                        let result =
                            post_json(&cfg, "/api/harness/chat", json!({ "text": text })).await;
                        match result {
                            Ok((code, body)) => {
                                let reply =
                                    best_reply_text(&body).unwrap_or_else(|| body.to_string());
                                lines.push(Line {
                                    role: "assistant",
                                    text: reply,
                                });
                                status = format!("ok {}", code);
                            }
                            Err(e) => {
                                lines.push(Line {
                                    role: "system",
                                    text: format!("request failed: {}", e),
                                });
                                status = "request failed".to_string();
                            }
                        }
                        if lines.len() > 2000 {
                            let keep_from = lines.len() - 2000;
                            lines.drain(0..keep_from);
                        }
                        let (w, h) = terminal::size().unwrap_or((120, 40));
                        scroll = max_scroll(&lines, w as usize, h as usize);
                    }
                    KeyCode::Char(c) => {
                        input.push(c);
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    scroll = scroll.saturating_sub(1);
                }
                MouseEventKind::ScrollDown => {
                    let (w, h) = terminal::size().unwrap_or((120, 40));
                    let max = max_scroll(&lines, w as usize, h as usize);
                    scroll = (scroll + 1).min(max);
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(())
}
