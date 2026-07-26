//! Fixed-shape tiling with overlap blending (AD-5).
//!
//! The QNN execution provider requires fixed input shapes, so arbitrary media
//! is normalised into tiles of one size with an overlap margin. Tiling is also
//! what bounds RAM and smooths thermal load.
//!
//! Reassembly is a weighted average over the overlap rather than a hard cut:
//! each tile's contribution ramps linearly from the edge of its margin, so a
//! stage that alters tiles slightly differently produces no visible seam. Where
//! the tiles agree, the weighted average returns the original value exactly.

use serde::{Deserialize, Serialize};

/// One tile's placement in the full image, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileSpec {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl TileSpec {
    /// Pixels in this tile.
    pub fn pixels(&self) -> usize {
        self.w as usize * self.h as usize
    }
}

/// Fixed-shape tiling with overlap blending. 512² for inpainting, 128² for
/// super-resolution, matching QuickSRNet's native design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tiler {
    pub tile: u32,
    pub overlap: u32,
}

impl Tiler {
    /// A tiler with the given tile size and overlap margin.
    pub fn new(tile: u32, overlap: u32) -> Self {
        Tiler { tile, overlap }
    }

    /// Distance between successive tile origins along one axis.
    fn stride(&self) -> u32 {
        self.tile.saturating_sub(self.overlap).max(1)
    }

    /// Tile origins along one axis of length `len`.
    fn starts(&self, len: u32) -> Vec<u32> {
        if len == 0 {
            return Vec::new();
        }
        if len <= self.tile {
            return vec![0];
        }
        let stride = self.stride();
        let mut starts = Vec::new();
        let mut s = 0u32;
        loop {
            starts.push(s);
            if s + self.tile >= len {
                break;
            }
            s += stride;
        }
        starts
    }

    /// Partition `w` by `h` into tiles of `self.tile`, overlapping by
    /// `self.overlap`, with partial tiles covering the right and bottom edges.
    ///
    /// The returned specs cover every pixel of the image exactly once or more.
    pub fn tile(&self, w: u32, h: u32) -> Vec<TileSpec> {
        let mut specs = Vec::new();
        for y in self.starts(h) {
            for x in self.starts(w) {
                specs.push(TileSpec {
                    x,
                    y,
                    w: self.tile.min(w - x),
                    h: self.tile.min(h - y),
                });
            }
        }
        specs
    }

    /// Blend weight for one coordinate inside a tile.
    ///
    /// The ramp only applies to an edge that meets another tile; an edge that
    /// is the image boundary keeps full weight, so the outermost pixels are not
    /// darkened towards nothing. The ramp starts at `1/(overlap+1)` rather than
    /// zero so every pixel carries strictly positive weight.
    fn axis_weight(&self, pos: u32, span: u32, at_low_edge: bool, at_high_edge: bool) -> f32 {
        if self.overlap == 0 {
            return 1.0;
        }
        let denominator = (self.overlap + 1) as f32;
        let mut weight = 1.0f32;
        if !at_low_edge && pos < self.overlap {
            weight = weight.min((pos + 1) as f32 / denominator);
        }
        if !at_high_edge && pos + self.overlap >= span {
            weight = weight.min((span - pos) as f32 / denominator);
        }
        weight
    }

    /// Reassemble tiles into a `w` by `h` image.
    ///
    /// The channel count is taken from the payload size of the first tile, so
    /// the same code path serves single-plane masks and interleaved RGB frames.
    /// A tile whose payload does not match its spec is skipped rather than
    /// corrupting the accumulator.
    pub fn blend(&self, tiles: &[(TileSpec, Vec<u8>)], w: u32, h: u32) -> Vec<u8> {
        let Some((first_spec, first_data)) = tiles.first() else {
            return Vec::new();
        };
        let pixels = first_spec.pixels();
        if pixels == 0 || first_data.len() % pixels != 0 {
            return Vec::new();
        }
        let channels = first_data.len() / pixels;
        let total = w as usize * h as usize;

        let mut acc = vec![0f32; total * channels];
        let mut weights = vec![0f32; total];

        for (spec, data) in tiles {
            if data.len() != spec.pixels() * channels {
                continue;
            }
            let at_left = spec.x == 0;
            let at_top = spec.y == 0;
            let at_right = spec.x + spec.w >= w;
            let at_bottom = spec.y + spec.h >= h;

            for ty in 0..spec.h {
                let wy = self.axis_weight(ty, spec.h, at_top, at_bottom);
                let image_y = (spec.y + ty) as usize;
                for tx in 0..spec.w {
                    let weight = wy * self.axis_weight(tx, spec.w, at_left, at_right);
                    let image_x = (spec.x + tx) as usize;
                    let dst = image_y * w as usize + image_x;
                    let src = ty as usize * spec.w as usize + tx as usize;
                    weights[dst] += weight;
                    for c in 0..channels {
                        acc[dst * channels + c] += weight * data[src * channels + c] as f32;
                    }
                }
            }
        }

        let mut out = vec![0u8; total * channels];
        for i in 0..total {
            let weight = weights[i];
            if weight <= 0.0 {
                continue;
            }
            for c in 0..channels {
                out[i * channels + c] = (acc[i * channels + c] / weight).round().clamp(0.0, 255.0) as u8;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic gradient with a diagonal component, so a misplaced tile
    /// shows up as a difference rather than cancelling out.
    fn gradient(w: u32, h: u32, channels: usize) -> Vec<u8> {
        let mut out = vec![0u8; w as usize * h as usize * channels];
        for y in 0..h as usize {
            for x in 0..w as usize {
                for c in 0..channels {
                    let v = (x * 5 + y * 3 + c * 31) % 256;
                    out[(y * w as usize + x) * channels + c] = v as u8;
                }
            }
        }
        out
    }

    fn cut(src: &[u8], w: u32, spec: &TileSpec, channels: usize) -> Vec<u8> {
        let mut out = vec![0u8; spec.pixels() * channels];
        for ty in 0..spec.h as usize {
            for tx in 0..spec.w as usize {
                let s = ((spec.y as usize + ty) * w as usize + spec.x as usize + tx) * channels;
                let d = (ty * spec.w as usize + tx) * channels;
                out[d..d + channels].copy_from_slice(&src[s..s + channels]);
            }
        }
        out
    }

    fn roundtrip(w: u32, h: u32, tile: u32, overlap: u32, channels: usize) {
        let tiler = Tiler::new(tile, overlap);
        let src = gradient(w, h, channels);
        let specs = tiler.tile(w, h);
        let cuts: Vec<(TileSpec, Vec<u8>)> = specs
            .iter()
            .map(|s| (*s, cut(&src, w, s, channels)))
            .collect();
        let out = tiler.blend(&cuts, w, h);

        assert_eq!(out.len(), src.len());
        for (i, (a, b)) in src.iter().zip(out.iter()).enumerate() {
            let delta = (*a as i32 - *b as i32).abs();
            assert!(
                delta <= 1,
                "byte {i} differs by {delta} ({a} vs {b}) at {w}x{h} tile {tile} overlap {overlap}"
            );
        }
    }

    #[test]
    fn tiling_then_blending_reproduces_a_gradient_within_one_lsb() {
        roundtrip(200, 140, 64, 16, 1);
        roundtrip(200, 140, 64, 16, 3);
        roundtrip(512, 512, 512, 32, 3);
        roundtrip(129, 65, 128, 32, 3);
        roundtrip(1, 1, 128, 32, 3);
    }

    #[test]
    fn tiles_cover_every_pixel() {
        let tiler = Tiler::new(64, 16);
        let (w, h) = (200u32, 140u32);
        let mut covered = vec![false; (w * h) as usize];
        for spec in tiler.tile(w, h) {
            assert!(spec.x + spec.w <= w);
            assert!(spec.y + spec.h <= h);
            for y in spec.y..spec.y + spec.h {
                for x in spec.x..spec.x + spec.w {
                    covered[(y * w + x) as usize] = true;
                }
            }
        }
        assert!(covered.iter().all(|c| *c), "tiling left a pixel uncovered");
    }

    #[test]
    fn interior_tiles_overlap_by_the_configured_margin() {
        let tiler = Tiler::new(64, 16);
        let specs = tiler.tile(200, 64);
        let row: Vec<u32> = specs.iter().map(|s| s.x).collect();
        assert_eq!(row, vec![0, 48, 96, 144]);
        // Successive origins step by tile - overlap, so neighbours share
        // exactly `overlap` columns.
        assert_eq!(0 + 64 - 48, tiler.overlap);
    }
}
