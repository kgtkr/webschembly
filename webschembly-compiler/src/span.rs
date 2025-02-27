#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: Pos,
    pub end: Pos,
}

impl Span {
    pub fn new(start: Pos, end: Pos) -> Self {
        Self { start, end }
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            start: self.start,
            end: other.end,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

impl Pos {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}
