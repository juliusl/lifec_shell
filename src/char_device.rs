use lifec::Component;
use lifec::HashMapStorage;
use tracing::Level;
use tracing::event;
use std::io::Cursor;
use tokio::io::AsyncRead;

/// Component that can be used to decode a sequence of terminal characters
///
#[derive(Component, Default)]
#[storage(HashMapStorage)]
pub struct CharDevice {
    // /// Decodes terminal character sequences
    // decoder: Decoder,
    /// The current buffer this device is writing to
    buffer: String,
    /// character counts per line
    line_info: Vec<usize>,
    /// cursor
    cursor: usize,
    /// line number
    line: usize,
    /// col number
    _col: usize,
}

impl CharDevice {
    /// Returns a read_only cursor for the current state of the buffer
    ///
    /// Not meant for polling for changes, but to make it more convenient for reading the current state of the device
    pub fn readonly_cursor(&self) -> impl AsyncRead {
        Cursor::new(self.buffer.as_bytes().to_vec())
    }

    /// Returns the number of lines in the buffer
    pub fn line_count(&self) -> usize {
        self.line_info.len()
    }

    /// Moves the cursor position up a line
    ///
    pub fn cursor_up(&mut self) {
        if self.line > 0 {
            self.line -= 1;
            self.goto_line(self.line);
        }
    }

    /// Moves the cursor down a line
    ///
    pub fn cursor_down(&mut self) {
        if self.line < self.line_info.len() - 1 {
            self.line += 1;
            self.goto_line(self.line);
        }
    }

    /// Moves the cursor left one character
    ///
    pub fn cursor_left(&mut self) {
        if self.cursor > 1 && !self.buffer.is_empty() {
            self.cursor -= 1;

            let check = self.cursor + 1;
            if let Some(b'\r') = &self.buffer.as_bytes().get(check) {
                if (self.line as i32) - 1 > 0 {
                    self.line -= 1;
                }
            }
        }
    }

    /// Moves the cursor right one character
    ///
    pub fn cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;

            let check = self.cursor - 1;
            if let Some(b'\r') = self.buffer.as_bytes().get(check) {
                self.line += 1;
            }
        }
    }

    /// Moves the character to line_no
    ///
    pub fn goto_line(&mut self, line_no: usize) {
        let chars = self.line_info.iter().take(line_no + 1).sum::<usize>();

        self.cursor = chars + line_no;
    }

    /// Writes the next character to the decoder, and internal buffer
    ///
    /// Updates internal counters
    pub fn write_char(&mut self, next: u8) {
        match char::from(next) {
            ref c if c.is_control() || c.is_ascii_control() => {
                if *c == '\u{7f}' || *c == '\u{00008}' {
                    self.backspace();
                } else if *c == '\r' {
                    self.buffer.insert(self.cursor, *c);
                    self.cursor += 1 as usize;
                    self.line += 1;
                }
            },
            ref c if c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_whitespace() => {
                self.buffer.insert(self.cursor, *c);
                self.cursor += 1 as usize;
            }
            c => {
                event!(Level::TRACE, "unhandled char {:#?}", c);
            }
        }

        self.line_info = self.buffer.split('\r').map(|l| l.len()).collect();
    }

    /// Returns the cursor's tail
    pub fn cursor_tail(&self) -> usize {
        if self.cursor > 1 {
            self.cursor - 1
        } else {
            0
        }
    }

    /// Returns the string before the cursor
    pub fn before_cursor(&self) -> impl AsRef<str> + '_ {
        if !self.buffer.is_empty() {
            &self.buffer[..self.cursor]
        } else {
            ""
        }
    }

    /// Returns the string after the cursor
    pub fn after_cursor(&self) -> impl AsRef<str> + '_ {
        if !self.buffer.is_empty() {
            &self.buffer[self.cursor_tail()..]
        } else {
            ""
        }
    }

    /// Returns the current output of the buffer
    pub fn output(&self) -> impl AsRef<str> + '_ {
        &self.buffer
    }

    /// Returns the current line nos
    pub fn line_nos(&self) -> impl AsRef<str> + '_ {
        (0..self.line_info.len())
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\r")
    }

    /// Returns the current line the cursor is on
    pub fn get_current_line(&self) -> Option<String> {
        self.get_line(self.line)
    }

    /// Returns the line at line_no
    pub fn get_line(&self, line_no: usize) -> Option<String> {
        self.buffer
            .split('\r')
            .collect::<Vec<_>>()
            .get(line_no)
            .and_then(|l| Some(l.to_string()))
    }

    /// Takes the current buffer, resetting the state and clearing the decoder for this device
    pub fn take_buffer(&mut self) -> String {
        let output = self.buffer.clone();
        self.buffer.clear();
        self.cursor = 0;
        self.line = 0;
        self.line_info.clear();
        output
    }

    fn backspace(&mut self) {
        if self.cursor > 0 && !self.buffer.is_empty() {
            self.cursor -= 1;
            match self.buffer.remove(self.cursor) {
                '\r' | '\n' => {
                    if self.line > 0 {
                        self.line -= 1;
                    }
                }
                _ => {}
            }
        }
    }
}
