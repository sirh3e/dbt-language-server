//! The character grid: `canvas.lua` in Rust. Boxes first, then each edge
//! as an orthogonal elbow from the right side of its parent to an arrow
//! at the left side of its child, turning at the gutter's middle. An edge
//! never draws over a box; crossing edges merge to `┼`; arrowheads win.

use std::collections::HashMap;

use super::layout::Placed;
use super::DagEdge;

/// Columns between two layers.
pub const GUTTER: i64 = 7;

const EDGE_CHARS: [char; 8] = ['─', '│', '┼', '▸', '╮', '╯', '╭', '╰'];

struct Grid {
    cells: Vec<Vec<char>>,
    boxed: Vec<Vec<bool>>,
}

impl Grid {
    fn new(width: i64, height: i64) -> Self {
        let (w, h) = (width.max(0) as usize, height.max(0) as usize);
        Grid {
            cells: vec![vec![' '; w]; h],
            boxed: vec![vec![false; w]; h],
        }
    }

    /// The 0-based index of a 1-based cell inside the grid.
    fn at(&self, x: i64, y: i64) -> Option<(usize, usize)> {
        let (col, row) = (usize::try_from(x - 1).ok()?, usize::try_from(y - 1).ok()?);
        (row < self.cells.len() && col < self.cells[row].len()).then_some((row, col))
    }

    fn put(&mut self, x: i64, y: i64, c: char) {
        if let Some((row, col)) = self.at(x, y) {
            self.cells[row][col] = c;
        }
    }

    fn put_edge(&mut self, x: i64, y: i64, c: char) {
        let Some((row, col)) = self.at(x, y) else {
            return;
        };
        if self.boxed[row][col] {
            return;
        }
        let current = self.cells[row][col];
        if current == ' ' || c == '▸' {
            self.cells[row][col] = c;
        } else if EDGE_CHARS.contains(&current) && current != c {
            self.cells[row][col] = '┼';
        }
    }

    fn draw_box(&mut self, b: &Placed) {
        let (x2, y2) = (b.x + b.w - 1, b.y + b.h - 1);
        self.put(b.x, b.y, '╭');
        self.put(x2, b.y, '╮');
        self.put(b.x, y2, '╰');
        self.put(x2, y2, '╯');
        for x in b.x + 1..x2 {
            self.put(x, b.y, '─');
            self.put(x, y2, '─');
        }
        for y in b.y + 1..y2 {
            self.put(b.x, y, '│');
            self.put(x2, y, '│');
        }
        for (i, line) in b.lines.iter().enumerate() {
            for (k, c) in line.chars().enumerate() {
                self.put(b.x + 2 + k as i64, b.y + 1 + i as i64, c);
            }
        }
        for y in b.y..=y2 {
            for x in b.x..=x2 {
                if let Some((row, col)) = self.at(x, y) {
                    self.boxed[row][col] = true;
                }
            }
        }
    }

    fn draw_edge(&mut self, from: &Placed, to: &Placed) {
        let (sx, sy) = (from.x + from.w, from.cy);
        let (tx, ty) = (to.x - 1, to.cy);
        if sy == ty {
            for x in sx..tx {
                self.put_edge(x, sy, '─');
            }
        } else {
            let down = sy < ty;
            let mid = (sx + 1).max(tx - GUTTER / 2);
            for x in sx..mid {
                self.put_edge(x, sy, '─');
            }
            self.put_edge(mid, sy, if down { '╮' } else { '╯' });
            let step = if down { 1 } else { -1 };
            let mut y = sy + step;
            while (down && y <= ty - step) || (!down && y >= ty - step) {
                self.put_edge(mid, y, '│');
                y += step;
            }
            self.put_edge(mid, ty, if down { '╰' } else { '╭' });
            for x in mid + 1..tx {
                self.put_edge(x, ty, '─');
            }
        }
        self.put_edge(tx, ty, '▸');
    }
}

/// Boxes, then edges, on a `width` × `height` grid; its rows right-trimmed.
pub fn draw(boxes: &[Placed], edges: &[DagEdge], width: i64, height: i64) -> Vec<String> {
    let mut grid = Grid::new(width, height);
    for b in boxes {
        grid.draw_box(b);
    }
    let by_id: HashMap<&str, &Placed> = boxes.iter().map(|b| (b.id.as_str(), b)).collect();
    for edge in edges {
        if let (Some(from), Some(to)) = (by_id.get(edge.from.as_str()), by_id.get(edge.to.as_str()))
        {
            grid.draw_edge(from, to);
        }
    }
    grid.cells
        .iter()
        .map(|row| row.iter().collect::<String>().trim_end().to_string())
        .collect()
}
