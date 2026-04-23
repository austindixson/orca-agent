use std::io::{self, Write};
use std::time::Duration;

use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, ClearType};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, MouseEventKind};
use serde_json::{json, Value};

use crate::config::OrcaConfig;
use crate::http_client::post_json;

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

fn one_line(s: &str) -> String {
    s.replace('\n', " ").replace('\r', " ")
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

fn render(lines: &[Line], input: &str, status: &str, scroll: usize, endpoint: &str) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    let (w, h) = terminal::size()?;
    let width = w as usize;
    let height = h as usize;

    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::All)
    )?;

    let header = format!("ORCA TUI  |  endpoint {}", endpoint);
    let hint = "Esc/Ctrl+C quit | Enter send | Up/Down scroll";

    writeln!(stdout, "{:=<1$}", "", width)?;
    writeln!(stdout, "{}", truncate_to(&header, width))?;
    writeln!(stdout, "{}", truncate_to(hint, width))?;
    writeln!(stdout, "{:=<1$}", "", width)?;

    let transcript_height = height.saturating_sub(7);
    let start = scroll.min(lines.len());
    let end = (start + transcript_height).min(lines.len());

    for line in &lines[start..end] {
        let prefix = match line.role {
            "user" => "> you: ",
            "assistant" => "< orca: ",
            "system" => "! system: ",
            _ => "  ",
        };
        let row = format!("{}{}", prefix, one_line(&line.text));
        writeln!(stdout, "{}", truncate_to(&row, width))?;
    }

    for _ in (end - start)..transcript_height {
        writeln!(stdout)?;
    }

    writeln!(stdout, "{:-<1$}", "", width)?;
    writeln!(stdout, "{}", truncate_to(status, width))?;
    let prompt = format!("> {}", input);
    write!(stdout, "{}", truncate_to(&prompt, width))?;
    stdout.flush()?;
    Ok(())
}

fn truncate_to(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    if width <= 1 {
        return "".to_string();
    }
    let mut out = String::new();
    for ch in s.chars().take(width.saturating_sub(1)) {
        out.push(ch);
    }
    out.push('…');
    out
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
    let mut scroll: usize = 0;

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
                        if scroll + 1 < lines.len() {
                            scroll += 1;
                        }
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

                        let result = post_json(&cfg, "/api/harness/chat", json!({ "text": text })).await;
                        match result {
                            Ok((code, body)) => {
                                let reply = best_reply_text(&body).unwrap_or_else(|| body.to_string());
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
                        if lines.len() > 5 {
                            scroll = lines.len().saturating_sub(5);
                        }
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
                    if scroll + 1 < lines.len() {
                        scroll += 1;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(())
}
