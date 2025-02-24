mod app;
mod csv;
mod find;
mod input;
mod ui;
#[allow(dead_code)]
mod util;
mod view;
use crate::app::App;

extern crate csv as sushi_csv;

use anyhow::{bail, Context, Result};
use clap::Parser;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::convert::TryInto;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use tempfile::NamedTempFile;
use tui::backend::CrosstermBackend;
use tui::Terminal;

struct SeekableFile {
    filename: Option<String>,
    inner_file: Option<NamedTempFile>,
}

impl SeekableFile {
    fn new(maybe_filename: &Option<String>) -> Result<SeekableFile> {
        let mut inner_file = NamedTempFile::new()?;
        let inner_file_res;

        if let Some(filename) = maybe_filename {
            let err = format!("Failed to open file: {}", filename);
            let mut f = File::open(filename).context(err)?;
            // If not seekable, it most likely is due to process substitution using
            // pipe - write out to a temp file to make it seekable
            if f.seek(SeekFrom::Start(0)).is_err() {
                let mut buffer: Vec<u8> = vec![];
                // TODO: could have read by chunks, yolo for now
                f.read_to_end(&mut buffer)?;
                inner_file.write_all(&buffer)?;
                inner_file_res = Some(inner_file);
            } else {
                inner_file_res = None;
            }
        } else {
            // Handle input from stdin
            let mut stdin = std::io::stdin();
            let mut buffer: Vec<u8> = vec![];
            stdin.read_to_end(&mut buffer)?;
            inner_file.write_all(&buffer)?;
            inner_file_res = Some(inner_file);
        }

        Ok(SeekableFile {
            filename: maybe_filename.clone(),
            inner_file: inner_file_res,
        })
    }

    fn filename(&self) -> &str {
        if let Some(f) = &self.inner_file {
            f.path().to_str().unwrap()
        } else {
            // If data is from stdin, then inner_file must be there
            self.filename.as_ref().unwrap()
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    /// CSV filename
    filename: Option<String>,

    /// Delimiter character (comma by default)
    #[clap(short, long)]
    delimiter: Option<String>,

    /// Show stats for debugging
    #[clap(long)]
    debug: bool,
}

fn parse_delimiter(args: &Args) -> Result<Option<u8>> {
    if let Some(s) = &args.delimiter {
        let mut chars = s.chars();
        let c = chars.next().context("Delimiter should not be empty")?;
        if !c.is_ascii() {
            bail!(
                "Delimiter should be within the ASCII range: {} is too fancy",
                c
            );
        }
        if chars.next().is_some() {
            bail!("Delimiter should be exactly one character, got {}", s);
        }
        Ok(Some(c.try_into()?))
    } else {
        Ok(None)
    }
}

fn run_csvlens() -> Result<()> {
    let args = Args::parse();

    let show_stats = args.debug;
    let delimiter = parse_delimiter(&args)?;

    let file = SeekableFile::new(&args.filename)?;
    let filename = file.filename();

    let mut app =
        App::new(filename, delimiter, args.filename, show_stats).context("Failed creating app")?;

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = app.main_loop(&mut terminal);

    // restore terminal
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    result
}

fn main() {
    if let Err(e) = run_csvlens() {
        println!("{}", e);
        std::process::exit(1);
    }
}
