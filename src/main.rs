use std::cmp::min;
use std::convert::TryInto;
use std::env::args;
use std::fs::File;
use std::io::{stdin, stdout, Write};
use std::vec::Vec;

use termion::clear;
use termion::cursor;
use termion::event::{Event, Key, MouseEvent};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use termion::terminal_size;

use ropey::Rope;

struct Editor {
    pub rope: Rope,

    line: usize,
    column: usize,
}

impl Editor {
    fn new(rope: Rope) -> Self {
        Self {
            rope,
            line: 0,
            column: 0,
        }
    }

    fn key(&mut self, key: Key) {
        match key {
            Key::Down => {
                if self.line < self.max_line() {
                    self.line += 1;
                }
            }
            Key::Right => {
                if self.column < self.max_column() {
                    self.column += 1;
                }
            }
            Key::Up => {
                self.line = self.line.saturating_sub(1);
            }
            Key::Left => {
                self.column = self.column.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn mouse(&mut self, mouse: MouseEvent, x: usize, y: usize) {
        match mouse {
            MouseEvent::Press(_, mouse_x, mouse_y) => {
                self.line = min(y + (mouse_y - 1) as usize, self.max_line());
                self.column = min(x + (mouse_x - 1) as usize, self.max_column());
            },
            _ => {}
        }
    }

    fn max_line(&self) -> usize {
        self.rope.len_lines().saturating_sub(1)
    }

    fn max_column(&self) -> usize {
        self.rope
            .line(self.line)
            .len_chars()
            // -2: -1 for line return + -1 for 0 based
            .saturating_sub(2)
    }

    fn column(&self) -> usize {
        min(self.column, self.max_column())
    }

    fn line(&self) -> usize {
        self.line
    }
}

struct TermRenderer {
    pub y: usize,
    pub x: usize,
    height: usize,
    width: usize,
}

impl TermRenderer {
    fn new() -> Self {
        let (width, height) = terminal_size().unwrap();
        Self {
            x: 0,
            y: 0,
            height: height as usize,
            width: width as usize,
        }
    }

    fn update<S>(&mut self, editor: &Editor, screen: &mut S, force: bool)
    where
        S: Write,
    {
        let mut need_update = force;
        if editor.line() < self.y {
            self.y = editor.line();
            need_update = true;
        }

        if editor.line() >= self.y + self.height {
            self.y = editor.line() - self.height + 1;
            need_update = true;
        }

        if editor.column() < self.x {
            self.x = editor.column();
            need_update = true;
        }

        if editor.column() >= self.x + self.width {
            self.x = editor.column() - self.width + 1;
            need_update = true;
        }

        if need_update {
            let mut screen = cursor::HideCursor::from(&mut *screen);

            let mut buffer = Vec::new();
            write!(buffer, "{}", clear::All).unwrap();
            write!(buffer, "{}", cursor::Goto(1, 1)).unwrap();

            let mut lines = editor
                .rope
                .lines()
                .map(|l| {
                    let max = l.len_chars().saturating_sub(1);
                    l.slice(min(self.x, max)..min(self.x + self.width, max))
                })
                .skip(self.y)
                .take(self.height as usize);

            if let Some(first) = lines.next() {
                write!(buffer, "\r{}", first).unwrap();
                for line in lines {
                    write!(buffer, "\n\r{}", line).unwrap();
                }
            }
            screen.write(&buffer).unwrap();
        }
        write!(
            screen,
            "{}",
            cursor::Goto(
                (editor.column() - self.x + 1).try_into().unwrap(),
                (editor.line() - self.y + 1).try_into().unwrap()
            )
        ).unwrap();
        screen.flush().unwrap();
    }
}

fn main() {
    let rope = if let Some(file) = args().nth(1) {
        Rope::from_reader(File::open(file).unwrap()).unwrap()
    } else {
        Rope::new()
    };

    let mut editor = Editor::new(rope);

    let mut renderer = TermRenderer::new();

    let stdin = stdin();
    let screen = stdout().into_raw_mode().unwrap();
    let screen = AlternateScreen::from(screen);
    let mut screen = MouseTerminal::from(screen);

    // Cursor shape, https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
    write!(screen, "\x1b[6 q").unwrap();

    renderer.update(&editor, &mut screen, true);

    for c in stdin.events() {
        let evt = c.unwrap();
        match evt {
            Event::Key(Key::Ctrl('c')) => break,
            Event::Key(key) => editor.key(key),
            Event::Mouse(mouse) => editor.mouse(mouse, renderer.x, renderer.y),
            _ => {}
        }
        renderer.update(&editor, &mut screen, false);
    }
}
