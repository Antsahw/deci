use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use std::fs;
use std::io::{self, Write};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;

// A simple Drop guard to ensure terminal state is ALWAYS restored even on panic
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = io::stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <filename>", args[0]);
        return Ok(());
    }

    let filename = &args[1];
    let mut memory_buffer = String::new();

    if let Ok(existing_content) = fs::read_to_string(filename) {
        memory_buffer = existing_content;
    }

    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-eighties.dark"];

    let extension = filename.split('.').last().unwrap_or("");
    let syntax = ps
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    // Initialize terminal safety setup
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let _guard = TerminalGuard; // Will restore terminal automatically if we crash

    let mut cursor_idx = memory_buffer.len();

    // Render loop helper
    let render = |buf: &str, cur: usize| -> std::io::Result<()> {
        print!("\x1b[H\x1b[J");
        print!("Ctrl+S to save | Ctrl+X / Ctrl+C to exit\r\n");

        // Dynamic but safe cursor mapping
        let mut target_line = 2;
        let mut target_col = 1;

        if let Some(sub_str) = buf.get(..cur) {
            target_line = sub_str.chars().filter(|&ch| ch == '\n').count() + 2;
            target_col = sub_str.split('\n').last().unwrap_or("").chars().count() + 1;
        }

        let mut highlighter = HighlightLines::new(syntax, theme);
        for line in buf.lines() {
            let ranges = highlighter.highlight_line(line, &ps).unwrap();
            let escaped_ansi = as_24_bit_terminal_escaped(&ranges[..], false);
            print!("{}\r\n", escaped_ansi);
        }

        if buf.ends_with('\n') {
            print!("\r\n");
        }

        print!("\x1b[0m\x1b[{};{}H", target_line, target_col);
        io::stdout().flush()?;
        Ok(())
    };

    render(&memory_buffer, cursor_idx)?;

    loop {
        if let Event::Key(key) = event::read()? {
            // Handle Global Shortcuts
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('s') => {
                        fs::write(filename, &memory_buffer)?;
                    }
                    KeyCode::Char('x') | KeyCode::Char('c') => {
                        break;
                    }
                    _ => {}
                }
                continue;
            }

            // Handle Input and Navigation
            match key.code {
                KeyCode::Up => {
                    if let Some(start_idx) = memory_buffer[..cursor_idx].rfind('\n') {
                        let col = cursor_idx - (start_idx + 1);
                        let prev_line_chunk = &memory_buffer[..start_idx];
                        let prev_line_start =
                            prev_line_chunk.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
                        let prev_line_len = start_idx - prev_line_start;
                        cursor_idx = prev_line_start + std::cmp::min(col, prev_line_len);
                    }
                }
                KeyCode::Down => {
                    if let Some(next_newline) = memory_buffer[cursor_idx..].find('\n') {
                        let global_next_newline = cursor_idx + next_newline;
                        let current_line_start = memory_buffer[..cursor_idx]
                            .rfind('\n')
                            .map(|idx| idx + 1)
                            .unwrap_or(0);
                        let col = cursor_idx - current_line_start;
                        let rest = &memory_buffer[global_next_newline + 1..];
                        let next_line_end = rest.find('\n').unwrap_or(rest.len());
                        cursor_idx = global_next_newline + 1 + std::cmp::min(col, next_line_end);
                    }
                }
                KeyCode::Left => {
                    if cursor_idx > 0 {
                        if let Some(c) = memory_buffer[..cursor_idx].chars().next_back() {
                            cursor_idx -= c.len_utf8();
                        }
                    }
                }
                KeyCode::Right => {
                    if cursor_idx < memory_buffer.len() {
                        if let Some(c) = memory_buffer[cursor_idx..].chars().next() {
                            cursor_idx += c.len_utf8();
                        }
                    }
                }
                KeyCode::Backspace => {
                    if cursor_idx > 0 {
                        if let Some(c) = memory_buffer[..cursor_idx].chars().next_back() {
                            let len = c.len_utf8();
                            memory_buffer.remove(cursor_idx - len);
                            cursor_idx -= len;
                        }
                    }
                }
                KeyCode::Enter => {
                    memory_buffer.insert(cursor_idx, '\n');
                    cursor_idx += 1;
                }
                KeyCode::Char(character) => {
                    // Match safe smart pairing brackets
                    match character {
                        '(' => {
                            memory_buffer.insert_str(cursor_idx, "()");
                            cursor_idx += 1;
                        }
                        '{' => {
                            memory_buffer.insert_str(cursor_idx, "{}");
                            cursor_idx += 1;
                        }
                        '[' => {
                            memory_buffer.insert_str(cursor_idx, "[]");
                            cursor_idx += 1;
                        }
                        '"' => {
                            memory_buffer.insert_str(cursor_idx, "\"\"");
                            cursor_idx += 1;
                        }
                        '\'' => {
                            memory_buffer.insert_str(cursor_idx, "''");
                            cursor_idx += 1;
                        }
                        _ => {
                            memory_buffer.insert(cursor_idx, character);
                            cursor_idx += character.len_utf8();
                        }
                    }
                }
                _ => {}
            }
            render(&memory_buffer, cursor_idx)?;
        }
    }

    Ok(())
}
