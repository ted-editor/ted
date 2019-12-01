use std::env::args;
use std::fmt::Display;
use std::fs::File;
use std::io::{stdin, stdout, Write};
use std::cmp::min;

use termion::clear;
use termion::cursor;
use termion::event::{Event, Key, MouseEvent};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use termion::terminal_size;

use ropey::Rope;

struct View {
    pub top_line: usize,
    pub height: u16,
    pub width: u16,
}

impl View {
    fn new(height: u16, width: u16) -> Self {
        Self {
            height,
            width,
            top_line: 0,
        }
    }

    fn resize(&mut self, height: u16, width: u16) {
        self.height = height;
        self.width = width;
    }

    fn draw<Screen, Lines>(&self, screen: &mut Screen, lines: Lines)
    where
        Screen: Write,
        Lines: Iterator,
        Lines::Item: Display,
    {
        let mut lines = lines.skip(self.top_line).take(self.height as usize);
        if let Some(first) = lines.next() {
            write!(screen, "\r{}", first);
            for line in lines {
                write!(screen, "\n\r{}", line);
            }
        }
    }
}

struct Editor<'a> {
    rope: &'a Rope,
    pub view: View,
    pub need_view_update: bool,
    line: u16,
    column: u16
}

impl<'a> Editor<'a> {
    fn new(rope: &'a Rope) -> Self {
        let (height, width) = terminal_size().unwrap();
        Self {
            rope,
            view: View::new(width, height),
            need_view_update: true,
            line: 0,
            column: 0
        }
    }

    fn draw<S: Write>(&mut self, screen: &mut S) {
        {
            let mut screen = cursor::HideCursor::from(&mut *screen);
            if self.need_view_update {
                write!(screen, "{}", clear::All).unwrap();
                write!(screen, "{}", cursor::Goto(1, 1));
                self.view.draw(&mut screen, self.rope.lines().map(|l| l.slice(..(l.len_chars() - 1))));
                self.need_view_update = false;
            }
            write!(screen, "{}", cursor::Goto(self.column() + 1, self.line + 1));
        }
        screen.flush().unwrap();
    }

    fn max_column(&self) -> usize {
        let chars = self.rope.line(self.view.top_line + self.line as usize).len_chars();
        // -2: -1 for line return + -1 for 0 based
        if chars > 2 { return chars - 2 } else { 0 }
    }

    fn column(&self) -> u16 {
        min(self.column as usize, self.max_column()) as u16
    }

    fn key(&mut self, key: Key) {
        match key {
            Key::Down => {
                if self.line as usize + self.view.top_line + 1 < self.rope.len_lines() {
                    if self.line == self.view.height {
                        self.view.top_line += 1;
                        self.need_view_update = true;
                    } else {
                        self.line += 1;
                    }
                }
            },
            Key::Up => {
                if self.line != 0 {
                    self.line -= 1;
                } else if self.view.top_line != 0 {
                    self.view.top_line -= 1;
                    self.need_view_update = true;
                }
            },
            Key::Left => {
                if self.column != 0 {
                    self.column -= 1;
                }
            },
            Key::Right => {
                if (self.column as usize) < self.max_column() {
                    self.column += 1;
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let rope = if let Some(file) = args().nth(1) {
        Rope::from_reader(File::open(file).unwrap()).unwrap()
    } else {
        Rope::new()
    };

    let mut editor = Editor::new(&rope);

    let stdin = stdin();
    let screen = stdout().into_raw_mode().unwrap();
    let screen = AlternateScreen::from(screen);
    let mut screen = MouseTerminal::from(screen);

    editor.draw(&mut screen);

    for c in stdin.events() {
        let evt = c.unwrap();
        match evt {
            Event::Key(Key::Ctrl('c')) => break,
            Event::Key(key) => editor.key(key),
            _ => {}
        }
        editor.draw(&mut screen);
    }
}
