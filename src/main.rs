mod app;
mod constants;
mod data;
mod formatters;
mod system_reader;
mod ui;

use app::{App, Tab};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use ui::ui;

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    let app = App::new();
    let res = run_app(&mut terminal, app);
    
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    
    if let Err(e) = res {
        eprintln!("{e:?}");
    }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let tick = Duration::from_millis(1000);
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &mut app))?;
        let timeout = tick.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => app.navigate(1),
                    KeyCode::Up | KeyCode::Char('k') => app.navigate(-1),
                    KeyCode::Tab => app.tab = app.tab.next(),
                    KeyCode::BackTab => app.tab = app.tab.prev(),
                    KeyCode::Char('1') => app.tab = Tab::Details,
                    KeyCode::Char('2') => app.tab = Tab::Traffic,
                    KeyCode::Char('3') => app.tab = Tab::Packets,
                    KeyCode::Char('f') => {
                        app.show_all = !app.show_all;
                        app.list_state.select(Some(0));
                    }
                    KeyCode::Char('r') => app.refresh(),
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick {
            app.refresh();
            last_tick = Instant::now();
        }
    }
}