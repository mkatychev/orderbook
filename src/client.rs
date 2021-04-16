use crate::event::*;
use std::{
    error::Error,
    io::{self},
};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Terminal,
};

use tonic::Request;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

mod event;

use orderbook::{orderbook_aggregator_client::*, Empty};

pub struct StatefulTable {
    state: TableState,
}

impl StatefulTable {
    fn new() -> StatefulTable {
        StatefulTable {
            state: TableState::default(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = OrderbookAggregatorClient::connect("http://[::1]:10000").await?;
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let events = Events::new();

    let mut table = StatefulTable::new();

    let mut stream = client
        .book_summary(Request::new(Empty {}))
        .await?
        .into_inner();

    print!("\x1B[2J"); // clear the screen first
    while let Some(summary) = stream.message().await? {
        if let Event::Input(key) = events.next()? {
            {
                break;
            }
        };

        terminal.draw(|f| {
            let rects = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .margin(5)
                .split(f.size());

            let normal_style = Style::default().bg(Color::Blue);
            let header_cells = ["ASKS", "BIDS"]
                .iter()
                .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));
            let header = Row::new(header_cells)
                .style(normal_style)
                .height(1)
                .bottom_margin(1);

            let len = std::cmp::min(summary.asks.len(), summary.bids.len());
            let asks = &summary.asks[..len];
            let bids = &summary.bids[..len];
            let mut zip: Vec<Vec<String>> = vec![];

            // zip vectors of asks and bids together
            for i in 0..len {
                zip.push(vec![
                    format!("{} - {}", asks[i].price.to_string(), asks[i].exchange),
                    format!("{} - {}", bids[i].price.to_string(), asks[i].exchange),
                ]);
            }
            let rows = zip.iter().map(|item| {
                let height = item
                    .iter()
                    .map(|content| content.chars().filter(|c| *c == '\n').count())
                    .max()
                    .unwrap_or(0)
                    + 1;
                let cells = item.iter().map(|c| Cell::from(c.clone()));
                Row::new(cells).height(height as u16).bottom_margin(1)
            });
            let t = Table::new(rows)
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("SPREAD: {}", summary.spread)),
                )
                .widths(&[
                    Constraint::Percentage(50),
                    Constraint::Length(30),
                    Constraint::Max(10),
                ]);
            f.render_stateful_widget(t, rects[0], &mut table.state);
        })?;
    }

    dbg!("done");
    Ok(())
}
