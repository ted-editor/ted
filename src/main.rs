use std::cmp::min;
use std::env::args;
use std::fs::File;
use std::io::{stdin, stdout, Write};
use std::vec::Vec;

use termion::clear;
use termion::cursor;
use termion::style;
use termion::event::{Event, Key, MouseEvent, MouseButton};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use termion::terminal_size;

use ropey::Rope;
use ropey::RopeSlice;

fn lines(rope: &Rope) -> usize {
    rope.len_lines().saturating_sub(1)
}

fn columns(line: RopeSlice) -> usize {
    let max = line.len_chars();
    if max > 0 && line.char(max - 1) == '\n'
    { max - 1 } else { max }
}

fn end(rope: &Rope) -> usize {
    let line = lines(rope);
    rope.line_to_char(line) + columns(rope.line(line))
}

struct Cursor {
    line: usize,
    col: usize,
}

enum Movement {
    Up(usize),
    Down(usize),
    Left(usize),
    Right(usize),
    Begin,
    End,
    LineBegin,
    LineEnd,
    Goto(usize, usize),
    GotoLine(usize),
    GotoCol(usize),
}

impl Cursor {
    fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    fn line(&self) -> usize {
        self.line
    }

    fn columns(&self, rope: &Rope) -> usize {
        columns(rope.line(self.line))
    }

    fn col(&self, rope: &Rope) -> usize {
        min(self.col, self.columns(rope))
    }

    fn pos(&self, rope: &Rope) -> usize {
        rope.line_to_char(self.line) + self.col(rope)
    }

    fn apply(&mut self, rope: &Rope, movement: Movement) {
        match movement {
            Movement::Up(n) => {
                self.line = self.line.saturating_sub(n);
            }
            Movement::Down(n) => {
                self.line = if self.line + n >= lines(rope) { lines(rope) } else { self.line + n };
            }
            Movement::Left(n) => {
                self.col = self.col(rope);
                for _ in 0..n {
                    if self.col > 0 {
                        self.col -= 1;
                    } else if self.line > 0 {
                        self.line -= 1;
                        self.col = self.columns(rope);
                    } else {
                        break
                    }
                }
            }
            Movement::Right(n) => {
                self.col = self.col(rope);
                for _ in 0..n {
                    if self.col < self.columns(rope) {
                        self.col += 1;
                    } else if self.line < lines(rope) {
                        self.line += 1;
                        self.col = 0;
                    } else {
                        break
                    }
                }
            }
            Movement::LineBegin => {
                if self.col == 0 { self.apply(rope, Movement::Up(1)) }
                else {  self.col = 0 }
            }
            Movement::LineEnd => {
                if self.col >= self.columns(rope) {
                    self.apply(rope, Movement::Down(1))
                }
                self.col = self.columns(rope);
            }
            Movement::Begin => {
                self.line = 0;
                self.col = 0;
            }
            Movement::End => {
                self.line = lines(rope);
                self.col = self.columns(rope);
            }
            Movement::Goto(line, col) => {
                self.line = min(line, lines(rope));
                self.col = min(col, self.columns(rope));
            }
            Movement::GotoLine(line) => {
                self.line = min(line, lines(rope));
            }
            Movement::GotoCol(col) => {
                self.col = min(col, self.columns(rope));
            }
        }
    }
}

struct Editor {
    pub rope: Rope,
    pub cursors: Vec<Cursor>,
}

impl Editor {
    fn new(rope: Rope) -> Self {
        let mut editor = Self {
            rope,
            cursors: Vec::with_capacity(4),
        };

        editor.cursors.push(Cursor::new(0 ,0));
        editor
    }

    fn line(&self) -> usize {
        if let Some(cursor) = self.cursors.first() { cursor.line() } else { 0 }
    }

    fn col(&self) -> usize {
        if let Some(cursor) = self.cursors.first() { cursor.col(&self.rope) } else { 0 }
    }

    fn key(&mut self, key: Key, height: usize) -> bool {
        match key {
            Key::Up => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::Up(1));
                }
                false
            }
            Key::Down => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::Down(1));
                }
                false
            }
            Key::Left => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::Left(1));
                }
                false
            }
            Key::Right => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::Right(1));
                }
                false
            }
            Key::Home => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::Begin);
                }
                false
            }
            Key::End => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::End);
                }
                false
            }
            Key::PageUp => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::Up(height));
                }
                false
            }
            Key::PageDown => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::Down(height));
                }
                false
            }
            Key::Ctrl('a') => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::LineBegin);
                }
                false
            }
            Key::Ctrl('e') => {
                for cursor in &mut self.cursors {
                    cursor.apply(&self.rope, Movement::LineEnd);
                }
                false
            }
            Key::Char(c) => {
                for cursor in &mut self.cursors {
                    self.rope.insert_char(cursor.pos(&self.rope), c);
                    cursor.apply(&self.rope, Movement::Right(1));
                }
                true
            }
            Key::Backspace => {
                for cursor in &mut self.cursors {
                    if cursor.pos(&self.rope) > 0 {
                        cursor.apply(&self.rope, Movement::Left(1));
                        let pos = cursor.pos(&self.rope);
                        self.rope.remove(pos..pos + 1);
                    }
                }
                true
            }
            Key::Delete => {
                for cursor in &mut self.cursors {
                    if cursor.pos(&self.rope) < end(&self.rope) {
                        cursor.apply(&self.rope, Movement::GotoCol(cursor.col(&self.rope)));
                        let pos = cursor.pos(&self.rope);
                        self.rope.remove(pos..pos + 1);
                    }
                }
                true
            }
            Key::Alt('j') => {
                if let Some(cursor) = self.cursors.first() {
                    if cursor.line > 0 {
                        self.cursors.insert(0, Cursor::new(cursor.line - 1, cursor.col));
                    }
                }
                true
            }
            Key::Alt('k') => {
                if let Some(cursor) = self.cursors.last() {
                    if cursor.line < lines(&self.rope) {
                        self.cursors.push(Cursor::new(cursor.line + 1, cursor.col));
                    }
                }
                true
            }
            Key::Esc => {
                self.cursors.drain(1..);
                true
            }
            _ => { false }
        }
    }

    fn mouse(&mut self, mouse: MouseEvent, x: usize, y: usize) {
        match mouse {
            MouseEvent::Press(MouseButton::Left, mouse_x, mouse_y) => {
                if let Some(cursor) = self.cursors.first_mut() {
                    cursor.apply(&self.rope,
                        Movement::Goto(y + (mouse_y - 1) as usize, x + (mouse_x - 1) as usize));
                }
            },
            _ => {}
        }
    }

    fn gotoline(&mut self, line: usize) {
        if let Some(cursor) = self.cursors.first_mut() {
            cursor.apply(&self.rope, Movement::GotoLine(line));
        }
    }

    fn save(&self, filename: String) {
        let mut file = File::create(filename).unwrap();
        for chunk in self.rope.chunks() {
            write!(file, "{}", chunk).unwrap();
        }
        file.sync_all().unwrap();
    }

    fn draw<W>(&self, w: &mut W, prefix: &str, line: RopeSlice, index: usize)
    where
        W: Write,
    {
        let mut cursors = self.cursors
            .iter().filter(|c| c.line == index)
            .map(|c| min(c.col, columns(line))).collect::<Vec<usize>>();

        cursors.sort();

        write!(w, "{}", prefix).unwrap();
        let last = cursors.into_iter().fold(0, |last, column| {
            write!(w, "{}{}{}{}", line.slice(last..column), style::Invert,
                   if column < line.len_chars() { line.char(column) } else { ' ' },
                   style::Reset).unwrap();
            column + 1
        });
        if last < line.len_chars() {
            write!(w, "{}", line.slice(last..)).unwrap();
        }
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

    fn update<S>(&mut self, editor: &Editor, screen: &mut S, draw: bool)
    where
        S: Write,
    {
        let mut need_update = true;
        if editor.line() < self.y {
            self.y = editor.line();
            need_update = true;
        }

        if editor.line() >= self.y + self.height {
            self.y = editor.line() - self.height + 1;
            need_update = true;
        }

        if editor.col() < self.x {
            self.x = editor.col();
            need_update = true;
        }

        if editor.col() >= self.x + self.width {
            self.x = editor.col() - self.width + 1;
            need_update = true;
        }

        if draw || need_update {
            let mut screen = cursor::HideCursor::from(&mut *screen);

            let mut buffer = Vec::with_capacity(self.width * self.height * 2);
            write!(buffer, "{}", clear::All).unwrap();
            write!(buffer, "{}", cursor::Goto(1, 1)).unwrap();
            write!(buffer, "{}", style::Reset).unwrap();

            let mut lines = editor
                .rope
                .lines()
                .map(|l| {
                    let max = columns(l);
                    l.slice(min(self.x, max)..min(self.x + self.width, max))
                })
                .skip(self.y)
                .take(self.height as usize);

            let mut ln = self.y;
            if let Some(first) = lines.next() {
                editor.draw(&mut buffer, "\r", first, ln);
                ln += 1;
                for line in lines {
                    editor.draw(&mut buffer, "\n\r", line, ln);
                    ln += 1;
                }
            }
            screen.write(&buffer).unwrap();
        }
        write!(
            screen,
            "{}",
            cursor::Hide
        ).unwrap();
        screen.flush().unwrap();
    }
}

fn main() {
    let rope = if let Some(path) = args().nth(1) {
        if let Ok(file) = File::open(path) {
            Rope::from_reader(file).unwrap()
        } else {
            Rope::new()
        }
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
        let draw = match evt {
            Event::Key(Key::Ctrl('q')) => break,
            Event::Key(Key::Ctrl('s')) => { if let Some(path) = args().nth(1) { editor.save(path); } false },
            Event::Key(key) => editor.key(key, renderer.height - 1),
            Event::Mouse(mouse) => { editor.mouse(mouse, renderer.x, renderer.y); false },
            _ => { false }
        };
        renderer.update(&editor, &mut screen, draw);
    }
}
