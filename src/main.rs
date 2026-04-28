mod app;
mod ui;

use std::io::{self, Read};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::App;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if !args.is_empty() && args[0] != "tui" {
        expand_stdin_args(&mut args)?;
        let mut app = App::new();
        match app.run_cli_command(&args) {
            Ok(lines) => {
                for line in lines {
                    println!("{}", line);
                }
                return Ok(());
            }
            Err(error) => {
                eprintln!("{}", error);
                std::process::exit(1);
            }
        }
    }
    if args.first().map(|arg| arg.as_str()) == Some("tui") {
        args.remove(0);
    }

    let _session = TerminalSession::enter()?;

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run(&mut terminal);

    terminal.show_cursor()?;

    result
}

struct TerminalSession;

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }

        Ok(Self)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();

        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    }
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    let tick_rate = Duration::from_millis(16); // ~60fps for smooth UI
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key_event) => app.handle_key(key_event),
                Event::Mouse(mouse_event) => app.handle_mouse(mouse_event),
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }

        if app.should_quit() {
            break;
        }
    }

    Ok(())
}

fn expand_stdin_args(args: &mut [String]) -> Result<()> {
    if !args.iter().any(|arg| arg == "-") {
        return Ok(());
    }

    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    for arg in args.iter_mut() {
        if arg == "-" {
            *arg = buffer.clone();
        }
    }
    Ok(())
}
