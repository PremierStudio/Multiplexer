//! Last-frame VT buffer for an in-app PTY pane.
//!
//! Grok's pager is a full TUI: alt screen, CUP, ED/EL, and `\r` overwrites.
//! A CSI-strip dump first looks blank, then becomes leftover ASCII. This
//! grid keeps one screen of cells so each redraw replaces the last frame.

/// One-shot helper size: wide enough that a single chunk does not wrap
/// unless the caller uses [`PtyFrame`] with the real PTY size.
pub const ONESHOT_COLS: u16 = 256;
pub const ONESHOT_ROWS: u16 = 128;

const TAB_STOP: usize = 8;

/// Last painted screen of a PTY. Incomplete ESC sequences are held
/// across [`PtyFrame::feed`] calls so a split CSI cannot leak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyFrame {
    cols: usize,
    rows: usize,
    cells: Vec<char>,
    row: usize,
    col: usize,
    saved_row: usize,
    saved_col: usize,
    pending: String,
}

impl Default for PtyFrame {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

impl PtyFrame {
    /// Empty grid. Zero cols or rows become 1.
    pub fn new(cols: u16, rows: u16) -> Self {
        let cols = cols.max(1) as usize;
        let rows = rows.max(1) as usize;
        Self {
            cols,
            rows,
            cells: vec![' '; cols * rows],
            row: 0,
            col: 0,
            saved_row: 0,
            saved_col: 0,
            pending: String::new(),
        }
    }

    pub fn size(&self) -> (u16, u16) {
        (self.cols as u16, self.rows as u16)
    }

    /// Zero-based cursor. Tests use this so CUP mutants cannot hide.
    pub fn cursor(&self) -> (u16, u16) {
        (self.col as u16, self.row as u16)
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let new_cols = cols.max(1) as usize;
        let new_rows = rows.max(1) as usize;
        if new_cols == self.cols && new_rows == self.rows {
            return;
        }
        let mut next = vec![' '; new_cols * new_rows];
        let copy_cols = self.cols.min(new_cols);
        let copy_rows = self.rows.min(new_rows);
        for r in 0..copy_rows {
            for c in 0..copy_cols {
                next[r * new_cols + c] = self.cells[r * self.cols + c];
            }
        }
        self.cells = next;
        self.cols = new_cols;
        self.rows = new_rows;
        self.row = self.row.min(new_rows - 1);
        self.col = self.col.min(new_cols - 1);
        self.saved_row = self.saved_row.min(new_rows - 1);
        self.saved_col = self.saved_col.min(new_cols - 1);
    }

    pub fn feed(&mut self, raw: &str) {
        if raw.is_empty() && self.pending.is_empty() {
            return;
        }
        let input = if self.pending.is_empty() {
            raw.to_owned()
        } else {
            let mut held = std::mem::take(&mut self.pending);
            held.push_str(raw);
            held
        };
        let mut rest = input.as_str();
        while !rest.is_empty() {
            let ch = rest.chars().next().expect("rest non-empty");
            if ch == '\u{1b}' {
                match consume_esc(rest) {
                    Ok(n) => {
                        self.apply_esc(&rest[..n]);
                        rest = &rest[n..];
                    }
                    Err(NeedMore) => {
                        self.pending = rest.to_owned();
                        return;
                    }
                }
            } else {
                self.put_char(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }
    }

    /// Every row, trailing spaces kept, so a TUI grid does not collapse.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        (0..self.rows)
            .map(|r| {
                let start = r * self.cols;
                let end = start + self.cols;
                self.cells[start..end].iter().collect()
            })
            .collect()
    }

    /// True when any cell is not a space. CSI-only frames stay blank.
    #[must_use]
    pub fn has_ink(&self) -> bool {
        self.cells.iter().any(|ch| *ch != ' ')
    }

    /// Trailing spaces and empty rows dropped so a blank screen stays empty.
    #[must_use]
    pub fn display(&self) -> String {
        let mut lines: Vec<String> = self
            .lines()
            .into_iter()
            .map(|line| line.trim_end().to_owned())
            .collect();
        while lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }

    fn apply_esc(&mut self, seq: &str) {
        debug_assert!(seq.starts_with('\u{1b}'));
        if seq.len() == 1 {
            return;
        }
        let second = seq[1..].chars().next().expect("seq longer than ESC");
        match second {
            '[' => self.apply_csi(&seq[2..]),
            '7' => {
                self.saved_row = self.row;
                self.saved_col = self.col;
            }
            '8' => {
                self.row = self.saved_row;
                self.col = self.saved_col;
            }
            'c' => self.reset(),
            'M' => self.reverse_index(),
            'D' => self.index(),
            'E' => {
                self.col = 0;
                self.index();
            }
            _ => {}
        }
    }

    fn apply_csi(&mut self, body_and_final: &str) {
        if body_and_final.is_empty() {
            return;
        }
        let bytes = body_and_final.as_bytes();
        let mut i = 0;
        let private = if matches!(bytes[0], b'?' | b'>' | b'=' | b'<') {
            i = 1;
            Some(bytes[0] as char)
        } else {
            None
        };
        let params_start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
            i += 1;
        }
        while i < bytes.len() && (0x20..=0x2F).contains(&bytes[i]) {
            i += 1;
        }
        if i >= bytes.len() {
            return;
        }
        let cmd = bytes[i] as char;
        let params = parse_params(std::str::from_utf8(&bytes[params_start..i]).unwrap_or(""));
        if private == Some('?') {
            if matches!(cmd, 'h' | 'l')
                && params
                    .iter()
                    .any(|&p| p == 1049 || p == 1047 || p == 47 || p == 1048)
            {
                self.clear_all();
                self.row = 0;
                self.col = 0;
            }
            return;
        }
        match cmd {
            'A' => self.cursor_up(mov_param(&params)),
            'B' => self.cursor_down(mov_param(&params)),
            'C' => self.cursor_right(mov_param(&params)),
            'D' => self.cursor_left(mov_param(&params)),
            'E' => {
                self.col = 0;
                self.cursor_down(mov_param(&params));
            }
            'F' => {
                self.col = 0;
                self.cursor_up(mov_param(&params));
            }
            'G' => {
                let n = mov_param(&params) as usize;
                self.col = n.saturating_sub(1).min(self.cols - 1);
            }
            'H' | 'f' => {
                let r = cup_param(&params, 0) as usize;
                let c = cup_param(&params, 1) as usize;
                self.row = r.saturating_sub(1).min(self.rows - 1);
                self.col = c.saturating_sub(1).min(self.cols - 1);
            }
            'J' => match ed_param(&params) {
                0 => self.erase_to_end(),
                1 => self.erase_to_start(),
                _ => self.clear_all(),
            },
            'K' => match ed_param(&params) {
                0 => self.erase_eol(),
                1 => self.erase_sol(),
                _ => self.erase_line(),
            },
            's' => {
                self.saved_row = self.row;
                self.saved_col = self.col;
            }
            'u' => {
                self.row = self.saved_row;
                self.col = self.saved_col;
            }
            _ => {}
        }
    }

    fn put_char(&mut self, ch: char) {
        match ch {
            '\r' => self.col = 0,
            '\n' => {
                self.col = 0;
                self.index();
            }
            '\t' => {
                let next = ((self.col / TAB_STOP) + 1) * TAB_STOP;
                self.col = next.min(self.cols - 1);
            }
            '\u{8}' => {
                if self.col > 0 {
                    self.col -= 1;
                }
            }
            '\u{0c}' => {
                self.clear_all();
                self.row = 0;
                self.col = 0;
            }
            '\0' | '\u{7}' | '\u{0b}' | '\u{0e}' | '\u{0f}' => {}
            ch if ch.is_control() => {}
            ch => self.write_cell(ch),
        }
    }

    fn write_cell(&mut self, ch: char) {
        if self.col >= self.cols {
            self.col = 0;
            self.index();
        }
        let i = self.row * self.cols + self.col;
        self.cells[i] = ch;
        self.col += 1;
    }

    fn index(&mut self) {
        if self.row + 1 < self.rows {
            self.row += 1;
        } else {
            self.scroll_up();
        }
    }

    fn reverse_index(&mut self) {
        if self.row > 0 {
            self.row -= 1;
        } else {
            self.scroll_down();
        }
    }

    fn scroll_up(&mut self) {
        self.cells.copy_within(self.cols.., 0);
        let start = (self.rows - 1) * self.cols;
        for cell in &mut self.cells[start..] {
            *cell = ' ';
        }
    }

    fn scroll_down(&mut self) {
        let tail = self.cols;
        let end = self.cells.len() - tail;
        self.cells.copy_within(0..end, tail);
        for cell in &mut self.cells[..self.cols] {
            *cell = ' ';
        }
    }

    fn cursor_up(&mut self, n: u16) {
        self.row = self.row.saturating_sub(n as usize);
    }

    fn cursor_down(&mut self, n: u16) {
        self.row = (self.row + n as usize).min(self.rows - 1);
    }

    fn cursor_right(&mut self, n: u16) {
        self.col = (self.col + n as usize).min(self.cols - 1);
    }

    fn cursor_left(&mut self, n: u16) {
        self.col = self.col.saturating_sub(n as usize);
    }

    fn clear_all(&mut self) {
        self.cells.fill(' ');
    }

    fn erase_eol(&mut self) {
        let start = self.row * self.cols + self.col;
        let end = self.row * self.cols + self.cols;
        for cell in &mut self.cells[start..end] {
            *cell = ' ';
        }
    }

    fn erase_sol(&mut self) {
        let start = self.row * self.cols;
        let end = start + self.col.min(self.cols - 1) + 1;
        for cell in &mut self.cells[start..end] {
            *cell = ' ';
        }
    }

    fn erase_line(&mut self) {
        let start = self.row * self.cols;
        let end = start + self.cols;
        for cell in &mut self.cells[start..end] {
            *cell = ' ';
        }
    }

    fn erase_to_end(&mut self) {
        self.erase_eol();
        let start = (self.row + 1) * self.cols;
        if start < self.cells.len() {
            for cell in &mut self.cells[start..] {
                *cell = ' ';
            }
        }
    }

    fn erase_to_start(&mut self) {
        let end = self.row * self.cols;
        if end > 0 {
            for cell in &mut self.cells[..end] {
                *cell = ' ';
            }
        }
        self.erase_sol();
    }

    fn reset(&mut self) {
        self.cells.fill(' ');
        self.row = 0;
        self.col = 0;
        self.saved_row = 0;
        self.saved_col = 0;
        self.pending.clear();
    }
}

struct NeedMore;

fn parse_params(raw: &str) -> Vec<u16> {
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(';')
        .map(|part| part.parse::<u16>().unwrap_or(0))
        .collect()
}

fn mov_param(params: &[u16]) -> u16 {
    match params.first() {
        None | Some(0) => 1,
        Some(n) => *n,
    }
}

fn cup_param(params: &[u16], index: usize) -> u16 {
    match params.get(index) {
        None | Some(0) => 1,
        Some(n) => *n,
    }
}

fn ed_param(params: &[u16]) -> u16 {
    params.first().copied().unwrap_or(0)
}

fn consume_esc(input: &str) -> Result<usize, NeedMore> {
    debug_assert!(input.starts_with('\u{1b}'));
    let rest = &input[1..];
    if rest.is_empty() {
        return Err(NeedMore);
    }
    let second = rest.chars().next().expect("rest non-empty");
    match second {
        '[' => consume_csi(rest),
        ']' => consume_osc(rest),
        'P' | '_' | '^' | 'X' => consume_string(rest),
        '(' | ')' | '*' | '+' => {
            let after = &rest[second.len_utf8()..];
            match after.chars().next() {
                None => Err(NeedMore),
                Some(designator) => Ok(1 + second.len_utf8() + designator.len_utf8()),
            }
        }
        _ => Ok(1 + second.len_utf8()),
    }
}

fn consume_csi(rest_after_esc: &str) -> Result<usize, NeedMore> {
    // rest_after_esc starts with '['
    let body = &rest_after_esc[1..];
    let bytes = body.as_bytes();
    let mut i = 0;
    if i < bytes.len() && matches!(bytes[i], b'?' | b'>' | b'=' | b'<') {
        i += 1;
    }
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
        i += 1;
    }
    while i < bytes.len() && (0x20..=0x2F).contains(&bytes[i]) {
        i += 1;
    }
    if i >= bytes.len() {
        return Err(NeedMore);
    }
    if !(0x40..=0x7E).contains(&bytes[i]) {
        return Ok(2 + i + 1);
    }
    Ok(2 + i + 1)
}

fn consume_osc(rest_after_esc: &str) -> Result<usize, NeedMore> {
    let body = &rest_after_esc[1..];
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x07 {
            return Ok(2 + i + 1);
        }
        if bytes[i] == 0x1b {
            if i + 1 >= bytes.len() {
                return Err(NeedMore);
            }
            if bytes[i + 1] == b'\\' {
                return Ok(2 + i + 2);
            }
            return Ok(2 + i);
        }
        i += 1;
    }
    Err(NeedMore)
}

fn consume_string(rest_after_esc: &str) -> Result<usize, NeedMore> {
    let body = &rest_after_esc[1..];
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x07 {
            return Ok(2 + i + 1);
        }
        if bytes[i] == 0x1b {
            if i + 1 >= bytes.len() {
                return Err(NeedMore);
            }
            if bytes[i + 1] == b'\\' {
                return Ok(2 + i + 2);
            }
            return Ok(2 + i);
        }
        i += 1;
    }
    Err(NeedMore)
}

/// Last-frame render of one PTY chunk. Incomplete sequences at the end
/// are dropped. The live pane should use a stateful [`PtyFrame`].
pub fn render_pty_chunk(raw: &str) -> String {
    let mut frame = PtyFrame::new(ONESHOT_COLS, ONESHOT_ROWS);
    frame.feed(raw);
    frame.display()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(cols: u16, rows: u16) -> PtyFrame {
        PtyFrame::new(cols, rows)
    }

    #[test]
    fn default_is_80x24() {
        let f = PtyFrame::default();
        assert_eq!(f.size(), (80, 24));
        assert_eq!(f.cursor(), (0, 0));
        assert!(!f.has_pending());
        assert_eq!(f.display(), "");
        assert_ne!(f.size(), (0, 0));
        assert_ne!(PtyFrame::new(0, 0).size(), (0, 0));
        assert_eq!(PtyFrame::new(0, 0).size(), (1, 1));
        assert_ne!(PtyFrame::new(2, 3).size(), (3, 2));
    }

    #[test]
    fn plain_text_is_the_frame() {
        let mut f = frame(20, 4);
        f.feed("hello");
        assert_eq!(f.display(), "hello");
        assert_eq!(f.cursor(), (5, 0));
        assert_ne!(f.display(), "HELLO");
        f.feed("");
        assert_eq!(f.display(), "hello");
    }

    #[test]
    fn lines_keep_full_grid_and_ink() {
        let mut f = frame(4, 3);
        assert!(!f.has_ink());
        assert_eq!(f.lines().len(), 3);
        assert_eq!(f.lines()[0], "    ");
        f.feed("ab");
        assert!(f.has_ink());
        assert_eq!(f.lines()[0], "ab  ");
        assert_eq!(f.lines()[1], "    ");
        assert_eq!(f.display(), "ab");
        assert_ne!(f.lines()[0], "ab");
        f.feed("\u{1b}[2J\u{1b}[H");
        assert!(!f.has_ink());
        assert_eq!(f.lines().len(), 3);
    }

    #[test]
    fn carriage_return_overwrites_line() {
        let mut f = frame(20, 2);
        f.feed("hello\rX");
        assert_eq!(f.display(), "Xello");
        assert_ne!(f.display(), "helloX");
        assert_ne!(f.display(), "hello");
        f.feed("\rYZ");
        assert_eq!(f.display(), "YZllo");
    }

    #[test]
    fn crlf_is_one_line_break() {
        let mut f = frame(20, 4);
        f.feed("one\r\ntwo");
        assert_eq!(f.display(), "one\ntwo");
        assert_eq!(f.cursor(), (3, 1));
        assert_ne!(f.display(), "onetwo");
        assert_ne!(f.display(), "one\r\ntwo");
    }

    #[test]
    fn csi_clear_drops_prior_cells() {
        let mut f = frame(20, 4);
        f.feed("old junk");
        f.feed("\u{1b}[2Jnew");
        assert_eq!(f.display(), "        new");
        assert_ne!(f.display(), "old junknew");
        assert_ne!(f.display(), "old junk");
        f.feed("\u{1b}[2J\u{1b}[Hhome");
        assert_eq!(f.display(), "home");
    }

    #[test]
    fn pager_redraw_replaces_last_frame() {
        let mut f = frame(40, 8);
        f.feed("AAAA\r\nBBBB");
        f.feed("\u{1b}[?1049h\u{1b}[H\u{1b}[2J\u{1b}[1;1HGrok\u{1b}[2;1Hready");
        assert_eq!(f.display(), "Grok\nready");
        assert!(!f.display().contains("AAAA"));
        assert!(!f.display().contains("BBBB"));
        f.feed("\u{1b}[2J\u{1b}[Hnext");
        assert_eq!(f.display(), "next");
        assert!(!f.display().contains("Grok"));
    }

    #[test]
    fn csi_home_then_write_replaces_origin() {
        let mut f = frame(20, 3);
        f.feed("abc");
        f.feed("\u{1b}[HX");
        assert_eq!(f.display(), "Xbc");
        f.feed("\u{1b}[1;1HY");
        assert_eq!(f.display(), "Ybc");
        f.feed("\u{1b}[2;1Hz");
        assert_eq!(f.display(), "Ybc\nz");
        assert_eq!(f.cursor(), (1, 1));
        assert_ne!(f.cursor(), (0, 0));
    }

    #[test]
    fn cup_zero_means_one() {
        let mut f = frame(10, 4);
        f.feed("\u{1b}[0;0Hq");
        assert_eq!(f.display(), "q");
        assert_eq!(f.cursor(), (1, 0));
        f.feed("\u{1b}[;3Hr");
        assert_eq!(f.display(), "q r");
        assert_eq!(f.cursor(), (3, 0));
    }

    #[test]
    fn csi_el_erases_to_eol() {
        let mut f = frame(10, 2);
        f.feed("abcdef");
        f.feed("\u{1b}[3G\u{1b}[K");
        assert_eq!(f.display(), "ab");
        assert_ne!(f.display(), "abcdef");
        assert_ne!(f.display(), "ab def");
    }

    #[test]
    fn csi_el_variants() {
        let mut f = frame(10, 2);
        f.feed("abcdef");
        f.feed("\u{1b}[4G\u{1b}[1K");
        assert_eq!(f.display(), "    ef");
        f.feed("\u{1b}[2K");
        assert_eq!(f.display(), "");
        f.feed("\u{1b}[Gxyz");
        assert_eq!(f.display(), "xyz");
        assert_ne!(f.display(), "abcdef");
        assert_ne!(f.display(), "   xyz");
    }

    #[test]
    fn csi_ed_variants() {
        let mut f = frame(6, 3);
        f.feed("aaaaaa\nbbbbbb\ncccccc");
        f.feed("\u{1b}[2;3H\u{1b}[0J");
        assert_eq!(f.display(), "aaaaaa\nbb");
        f.feed("\u{1b}[Haaaaaa\nbbbbbb\ncccccc");
        f.feed("\u{1b}[2;3H\u{1b}[1J");
        assert_eq!(f.display(), "\n   bbb\ncccccc");
        f.feed("\u{1b}[3J");
        assert_eq!(f.display(), "");
        assert_ne!(f.display(), "aaaaaa");
    }

    #[test]
    fn cursor_moves_abcd() {
        let mut f = frame(10, 5);
        f.feed("\u{1b}[3;4H");
        assert_eq!(f.cursor(), (3, 2));
        f.feed("\u{1b}[2A");
        assert_eq!(f.cursor(), (3, 0));
        f.feed("\u{1b}[B");
        assert_eq!(f.cursor(), (3, 1));
        f.feed("\u{1b}[3C");
        assert_eq!(f.cursor(), (6, 1));
        f.feed("\u{1b}[2D");
        assert_eq!(f.cursor(), (4, 1));
        f.feed("\u{1b}[E");
        assert_eq!(f.cursor(), (0, 2));
        f.feed("\u{1b}[F");
        assert_eq!(f.cursor(), (0, 1));
        assert_ne!(f.cursor(), (0, 0));
        f.feed("\u{1b}[A\u{1b}[A\u{1b}[A");
        assert_eq!(f.cursor(), (0, 0));
    }

    #[test]
    fn sgr_and_osc_do_not_print() {
        let mut f = frame(20, 2);
        f.feed("\u{1b}[32mhi\u{1b}[0m\u{1b}[38;2;255;0;0m!\u{1b}]0;title\u{7}ok");
        assert_eq!(f.display(), "hi!ok");
        assert!(!f.display().contains('\u{1b}'));
        assert!(!f.display().contains("32"));
        assert!(!f.display().contains("title"));
        assert_ne!(f.display(), "hi!");
    }

    #[test]
    fn split_csi_does_not_leak() {
        let mut f = frame(20, 2);
        f.feed("\u{1b}[32");
        assert!(f.has_pending());
        assert_eq!(f.display(), "");
        assert_ne!(f.display(), "[32");
        f.feed("mOK");
        assert!(!f.has_pending());
        assert_eq!(f.display(), "OK");
        assert_ne!(f.display(), "mOK");
        assert_ne!(f.display(), "[32mOK");
    }

    #[test]
    fn split_esc_then_clear() {
        let mut f = frame(20, 2);
        f.feed("keep\u{1b}");
        assert!(f.has_pending());
        assert_eq!(f.display(), "keep");
        f.feed("[2J\u{1b}[Hgone");
        assert_eq!(f.display(), "gone");
        assert!(!f.display().contains("keep"));
    }

    #[test]
    fn split_osc_does_not_leak() {
        let mut f = frame(20, 2);
        f.feed("\u{1b}]0;abc");
        assert!(f.has_pending());
        assert_eq!(f.display(), "");
        f.feed("def\u{7}xy");
        assert_eq!(f.display(), "xy");
        assert!(!f.display().contains("abc"));
        assert!(!f.has_pending());
    }

    #[test]
    fn ris_resets_grid() {
        let mut f = frame(10, 2);
        f.feed("zz\u{1b}cAA");
        assert_eq!(f.display(), "AA");
        assert_eq!(f.cursor(), (2, 0));
        assert!(!f.has_pending());
        assert_ne!(f.display(), "zzAA");
    }

    #[test]
    fn backspace_and_tab() {
        let mut f = frame(16, 2);
        f.feed("ab\u{8}c");
        assert_eq!(f.display(), "ac");
        f.feed("\r\tX");
        assert_eq!(f.display(), "ac      X");
        assert_eq!(f.cursor(), (9, 0));
        assert_ne!(f.display(), "acX");
    }

    #[test]
    fn wrap_at_width_and_scroll() {
        let mut f = frame(4, 2);
        f.feed("abcdEF");
        assert_eq!(f.display(), "abcd\nEF");
        f.feed("ghij");
        assert_eq!(f.display(), "EFgh\nij");
        assert_ne!(f.display(), "abcd\nEF");
        assert_ne!(f.display().lines().count(), 3);
    }

    #[test]
    fn form_feed_clears() {
        let mut f = frame(8, 2);
        f.feed("stay\u{0c}go");
        assert_eq!(f.display(), "go");
        assert_ne!(f.display(), "staygo");
    }

    #[test]
    fn save_restore_cursor() {
        let mut f = frame(10, 4);
        f.feed("\u{1b}[2;3H\u{1b}7\u{1b}[H.\u{1b}8x");
        assert_eq!(f.display(), ".\n  x");
        assert_eq!(f.cursor(), (3, 1));
        f.feed("\u{1b}[1;1H\u{1b}[s\u{1b}[3;1H\u{1b}[uy");
        assert_eq!(f.display(), "y\n  x");
    }

    #[test]
    fn resize_preserves_overlap() {
        let mut f = frame(4, 2);
        f.feed("ab\ncd");
        f.resize(6, 3);
        assert_eq!(f.size(), (6, 3));
        assert_eq!(f.display(), "ab\ncd");
        f.resize(1, 1);
        assert_eq!(f.display(), "a");
        assert_eq!(f.size(), (1, 1));
        assert_eq!(f.cursor(), (0, 0));
        f.feed("Z");
        assert_eq!(f.display(), "Z");
        f.resize(1, 1);
        assert_eq!(f.size(), (1, 1));
        assert_eq!(f.cursor(), (1, 0));
        assert_ne!(f.size(), (4, 2));
    }

    #[test]
    fn resize_one_axis_and_clamp_saved_cursor() {
        let mut f = frame(4, 3);
        f.feed("ab\ncd\nef");
        f.resize(8, 3);
        assert_eq!(f.size(), (8, 3));
        assert_eq!(f.display(), "ab\ncd\nef");
        f.feed("\u{1b}[1;6HX");
        assert_eq!(f.display().lines().next().unwrap(), "ab   X");
        f.resize(8, 5);
        assert_eq!(f.size(), (8, 5));
        f.feed("\u{1b}[4;1HY");
        let grown = f.display();
        let grown_lines: Vec<&str> = grown.lines().collect();
        assert_eq!(grown_lines.get(3).copied(), Some("Y"));
        assert_ne!(grown_lines.get(2).copied(), Some("Y"));
        f.feed("\u{1b}[3;8H\u{1b}7");
        f.resize(2, 1);
        assert_eq!(f.size(), (2, 1));
        assert_eq!(f.cursor(), (1, 0));
        f.feed("\u{1b}8Q");
        assert_eq!(f.display(), "aQ");
        assert_eq!(f.cursor(), (2, 0));
        assert_ne!(f.size(), (8, 5));
    }

    #[test]
    fn reverse_index_scrolls_down() {
        let mut f = frame(3, 2);
        f.feed("aa\nbb");
        f.feed("\u{1b}[H\u{1b}Mcc");
        assert_eq!(f.display(), "cc\naa");
        assert!(!f.display().contains("bb"));
    }

    #[test]
    fn index_and_nel() {
        let mut f = frame(4, 3);
        f.feed("hi\u{1b}Dxx");
        assert_eq!(f.display(), "hi\n  xx");
        f.feed("\u{1b}Eyy");
        assert_eq!(f.display(), "hi\n  xx\nyy");
        assert_ne!(f.display(), "hixx");
        let mut mid = frame(10, 3);
        mid.feed("ab\u{1b}Ecd");
        assert_eq!(mid.display(), "ab\ncd");
        assert_ne!(mid.display(), "abcd");
        assert_ne!(mid.display(), "ab  cd");
    }

    #[test]
    fn each_alt_screen_code_clears_and_homes() {
        for code in [1049_u16, 1047, 1048, 47] {
            let mut f = frame(8, 2);
            f.feed("keep");
            assert_eq!(f.display(), "keep");
            f.feed(&format!("\u{1b}[?{code}h"));
            assert_eq!(f.display(), "", "enter {code}");
            f.feed("A");
            assert_eq!(f.display(), "A", "home after {code}");
            f.feed("xyz");
            f.feed(&format!("\u{1b}[?{code}l"));
            assert_eq!(f.display(), "", "leave {code}");
        }
        let mut f = frame(8, 2);
        f.feed("keep\u{1b}[?25l");
        assert_eq!(f.display(), "keep");
        assert_ne!(f.display(), "");
    }

    #[test]
    fn csi_intermediate_is_ignored() {
        let mut f = frame(12, 2);
        f.feed("\u{1b}[2 qHi");
        assert_eq!(f.display(), "Hi");
        assert_ne!(f.display(), "2 qHi");
        assert_ne!(f.display(), "qHi");
    }

    #[test]
    fn charset_and_bel_ignored() {
        let mut f = frame(10, 2);
        f.feed("\u{1b}(Ba\u{7}b");
        assert_eq!(f.display(), "ab");
        assert_ne!(f.display(), "(Bab");
    }

    #[test]
    fn oneshot_does_not_wrap_long_line() {
        let line = "x".repeat(100);
        assert_eq!(render_pty_chunk(&line), line);
        assert_eq!(render_pty_chunk(&line).lines().count(), 1);
        assert_ne!(render_pty_chunk(&line).lines().count(), 2);
        assert_eq!(ONESHOT_COLS, 256);
        assert_eq!(ONESHOT_ROWS, 128);
        assert_ne!(ONESHOT_COLS, 80);
    }

    #[test]
    fn display_never_contains_esc() {
        let mut f = frame(20, 3);
        f.feed("\u{1b}[1;31mred\u{1b}[0m\u{1b}[2K\u{1b}[Hok");
        assert!(!f.display().contains('\u{1b}'));
        assert!(f.display().contains("ok"));
    }

    #[test]
    fn braille_spinner_overwrites() {
        let mut f = frame(8, 1);
        f.feed("⠋ work");
        f.feed("\r⠙ work");
        f.feed("\r⠹ work");
        assert_eq!(f.display(), "⠹ work");
        assert!(!f.display().contains('⠋'));
        assert_ne!(f.display(), "⠋ work⠙ work⠹ work");
    }

    #[test]
    fn hide_cursor_is_noop() {
        let mut f = frame(8, 2);
        f.feed("\u{1b}[?25lHi\u{1b}[?25h");
        assert_eq!(f.display(), "Hi");
        assert_ne!(f.display(), "25lHi");
    }

    #[test]
    fn osc_st_terminator() {
        let mut f = frame(8, 2);
        f.feed("\u{1b}]0;t\u{1b}\\Z");
        assert_eq!(f.display(), "Z");
        assert_ne!(f.display(), "tZ");
    }

    #[test]
    fn dcs_string_is_ignored() {
        let mut f = frame(8, 2);
        f.feed("\u{1b}P1$t\u{1b}\\ok");
        assert_eq!(f.display(), "ok");
        assert_ne!(f.display(), "1$tok");
        assert_ne!(f.display(), "P1$tok");
    }
}
