//! Generic, app-agnostic tiled layout machinery for the CYD panel.
//!
//! The CYD draws into a single shared [`PixelBuffer`](crate::PixelBuffer) that is
//! flushed in pieces. These types describe *where* those pieces live in screen
//! coordinates and *how big* the shared buffer must be, without knowing anything
//! about what an app draws into them.
//!
//! The primary type is [`TileGrid`]: it splits a rectangular body region into
//! a grid of fixed-size tiles, clipping the final column/row to the region edges.
//! [`Region`] describes a single rectangle (for example a full-width text
//! band), and [`max_usize`] combines pixel counts so a shared buffer can be sized
//! as the max of every frame an app flushes.

use embedded_graphics::prelude::{Point, Size};

/// `const fn` maximum of two `usize` values.
///
/// Useful for sizing a shared `PixelBuffer<N>` as the largest of several frame
/// pixel counts, e.g. `max_usize(text_band.pixel_count(), grid.max_tile_pixels())`.
#[must_use]
pub const fn max_usize(first: usize, second: usize) -> usize {
    if first > second { first } else { second }
}

/// A single rectangular screen region in physical-screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub top_left: Point,
    pub size: Size,
}

impl Region {
    #[must_use]
    pub const fn new(top_left: Point, size: Size) -> Self {
        Self { top_left, size }
    }

    /// Number of pixels the region covers (`width * height`).
    #[must_use]
    pub const fn pixel_count(&self) -> usize {
        self.size.width as usize * self.size.height as usize
    }
}

/// A single tile produced by [`TileGrid`], in physical-screen coordinates.
///
/// The final column/row of a grid may be narrower/shorter than the nominal tile
/// size when the body region is not an exact multiple of the tile size, so always
/// use `size` rather than the grid's `tile_size` when allocating a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub top_left: Point,
    pub size: Size,
}

/// A rectangular body region split into a grid of fixed-size tiles.
///
/// `top_left` and `size` describe the region in screen coordinates; `tile_size`
/// is the nominal size of each tile. The final column and row are clipped to the
/// region's right and bottom edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileGrid {
    pub top_left: Point,
    pub size: Size,
    pub tile_size: Size,
}

impl TileGrid {
    #[must_use]
    pub const fn new(top_left: Point, size: Size, tile_size: Size) -> Self {
        Self {
            top_left,
            size,
            tile_size,
        }
    }

    /// Number of tile columns needed to cover the region width (ceiling division).
    #[must_use]
    pub const fn columns(&self) -> usize {
        let region_width = self.size.width as usize;
        let tile_width = self.tile_size.width as usize;
        (region_width + tile_width - 1) / tile_width
    }

    /// Number of tile rows needed to cover the region height (ceiling division).
    #[must_use]
    pub const fn rows(&self) -> usize {
        let region_height = self.size.height as usize;
        let tile_height = self.tile_size.height as usize;
        (region_height + tile_height - 1) / tile_height
    }

    /// Largest pixel count any single tile can have.
    ///
    /// The biggest tile is the top-left one, whose dimensions are the tile size
    /// clipped to the region (in case the region is smaller than one tile).
    #[must_use]
    pub const fn max_tile_pixels(&self) -> usize {
        let widest = min_usize(self.tile_size.width as usize, self.size.width as usize);
        let tallest = min_usize(self.tile_size.height as usize, self.size.height as usize);
        widest * tallest
    }

    /// The tile at `(column, row)`, or `None` if it lies outside the region.
    #[must_use]
    pub fn tile(&self, column: usize, row: usize) -> Option<Tile> {
        let tile_width = self.tile_size.width as usize;
        let tile_height = self.tile_size.height as usize;
        let column_offset = column * tile_width;
        let row_offset = row * tile_height;

        let region_width = self.size.width as usize;
        let region_height = self.size.height as usize;
        if column_offset >= region_width || row_offset >= region_height {
            return None;
        }

        let width = min_usize(tile_width, region_width - column_offset);
        let height = min_usize(tile_height, region_height - row_offset);
        Some(Tile {
            top_left: Point::new(
                self.top_left.x + column_offset as i32,
                self.top_left.y + row_offset as i32,
            ),
            size: Size::new(width as u32, height as u32),
        })
    }

    /// Iterate over all tiles in row-major order (each row left-to-right).
    #[must_use]
    pub fn tiles(&self) -> TileIter {
        TileIter {
            grid: *self,
            column: 0,
            row: 0,
        }
    }
}

/// Iterator over the tiles of a [`TileGrid`], yielded in row-major order.
pub struct TileIter {
    grid: TileGrid,
    column: usize,
    row: usize,
}

impl Iterator for TileIter {
    type Item = Tile;

    fn next(&mut self) -> Option<Self::Item> {
        let columns = self.grid.columns();
        let rows = self.grid.rows();
        loop {
            if self.row >= rows {
                return None;
            }
            let tile = self.grid.tile(self.column, self.row);
            self.column += 1;
            if self.column >= columns {
                self.column = 0;
                self.row += 1;
            }
            if tile.is_some() {
                return tile;
            }
        }
    }
}

const fn min_usize(first: usize, second: usize) -> usize {
    if first < second { first } else { second }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Body region used by the dance app: 240×286 starting just below a 34 px
    // text band, tiled in 80×96 cells.
    const BODY_GRID: TileGrid = TileGrid::new(
        Point::new(0, 34),
        Size::new(240, 286),
        Size::new(80, 96),
    );

    #[test]
    fn exact_fit_columns_and_rows() {
        assert_eq!(BODY_GRID.columns(), 3);
        assert_eq!(BODY_GRID.rows(), 3);
    }

    #[test]
    fn final_row_is_clipped() {
        // Origin y = 34, region height 286 → last row (row 2) starts at offset
        // 192 and is clipped from 96 to 94 px high.
        let last_row = BODY_GRID.tile(0, 2).expect("tile (0, 2) is in range");
        assert_eq!(last_row.top_left, Point::new(0, 34 + 192));
        assert_eq!(last_row.size.height, 94);
        assert_eq!(last_row.size.width, 80);
    }

    #[test]
    fn final_column_and_row_clipping_for_uneven_dimensions() {
        // 250×290 region in 80×96 tiles: 4 columns (last 10 px) and 4 rows (last 2 px).
        let grid = TileGrid::new(Point::new(5, 7), Size::new(250, 290), Size::new(80, 96));
        assert_eq!(grid.columns(), 4);
        assert_eq!(grid.rows(), 4);

        let last_column = grid.tile(3, 0).expect("tile (3, 0) is in range");
        assert_eq!(last_column.top_left, Point::new(5 + 240, 7));
        assert_eq!(last_column.size.width, 10);
        assert_eq!(last_column.size.height, 96);

        let last_row = grid.tile(0, 3).expect("tile (0, 3) is in range");
        assert_eq!(last_row.size.height, 2);

        let corner = grid.tile(3, 3).expect("tile (3, 3) is in range");
        assert_eq!(corner.size, Size::new(10, 2));

        // Out of range in either axis is None.
        assert_eq!(grid.tile(4, 0), None);
        assert_eq!(grid.tile(0, 4), None);
    }

    #[test]
    fn max_tile_pixels_is_full_tile() {
        assert_eq!(BODY_GRID.max_tile_pixels(), 80 * 96);

        // Region smaller than one tile clips the reported max.
        let small = TileGrid::new(Point::new(0, 0), Size::new(40, 50), Size::new(80, 96));
        assert_eq!(small.max_tile_pixels(), 40 * 50);
    }

    #[test]
    fn text_band_pixel_count() {
        let text_band = Region::new(Point::new(0, 0), Size::new(240, 34));
        assert_eq!(text_band.pixel_count(), 8160);
    }

    #[test]
    fn tiles_iter_visits_every_tile_once() {
        let tiles: heapless::Vec<Tile, 16> = BODY_GRID.tiles().collect();
        assert_eq!(tiles.len(), 9);
        // Row-major: first three share the same y, then the next row.
        assert_eq!(tiles[0].top_left, Point::new(0, 34));
        assert_eq!(tiles[1].top_left, Point::new(80, 34));
        assert_eq!(tiles[2].top_left, Point::new(160, 34));
        assert_eq!(tiles[3].top_left, Point::new(0, 34 + 96));
    }
}
