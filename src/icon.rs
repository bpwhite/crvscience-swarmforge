// Copyright (C) 2026 CRV Science Network (https://crvscience.com)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://gnu.org>.

#![forbid(unsafe_code)]

/// Renders a deterministic `size x size` RGBA icon matching `assets/swarmforge.ico`.
pub fn render_icon_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;

    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / s * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / s * 2.0 - 1.0;
            let idx = ((y * size + x) * 4) as usize;

            let rx = u.abs() - 0.72;
            let ry = v.abs() - 0.72;
            let d_tile = rx.max(0.0).hypot(ry.max(0.0)) + rx.max(ry).min(0.0) - 0.18;
            if d_tile > 0.0 {
                continue;
            }

            let (mut r, mut g, mut b) = if d_tile > -0.06 {
                (38u8, 72u8, 115u8)
            } else {
                (14u8, 23u8, 38u8)
            };

            let iso = u.abs() * 0.85 + v.abs() * 1.25;
            if iso < 0.72 {
                if iso > 0.65 {
                    (r, g, b) = (80, 220, 250);
                } else {
                    (r, g, b) = (25, 50, 82);
                    let g1 = ((u * 0.85 + v * 1.25) * 3.0).rem_euclid(1.0);
                    let g2 = ((u * 0.85 - v * 1.25) * 3.0).rem_euclid(1.0);
                    if g1 < 0.08 || g1 > 0.92 || g2 < 0.08 || g2 > 0.92 {
                        (r, g, b) = (38, 88, 138);
                    }
                }
            }

            let dist_c = u.hypot(v);
            let ang = v.atan2(u);
            let gear_r = 0.25 + 0.06 * (ang * 6.0).cos();
            if dist_c < gear_r && dist_c > 0.10 {
                (r, g, b) = (250, 215, 90);
            } else if dist_c <= 0.10 {
                (r, g, b) = (60, 235, 145);
            }

            for (nx, ny) in [(0.0f32, -0.56f32), (0.0, 0.56), (-0.58, 0.0), (0.58, 0.0)] {
                let dn = (u - nx).hypot(v - ny);
                if dn < 0.11 {
                    if dn < 0.045 {
                        (r, g, b) = (240, 252, 255);
                    } else {
                        (r, g, b) = (80, 220, 250);
                    }
                }
            }

            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = 255;
        }
    }

    rgba
}

/// Returns the 64x64 RGBA window/taskbar icon for `eframe::egui::ViewportBuilder::with_icon`.
pub fn window_icon_data() -> egui::IconData {
    let size = 64u32;
    egui::IconData {
        rgba: render_icon_rgba(size),
        width: size,
        height: size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_icon_data_has_valid_dimensions_and_alpha() {
        let icon = window_icon_data();
        assert_eq!(icon.width, 64);
        assert_eq!(icon.height, 64);
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
        let opaque = icon.rgba.chunks_exact(4).filter(|px| px[3] == 255).count();
        let transparent = icon.rgba.chunks_exact(4).filter(|px| px[3] == 0).count();
        assert!(opaque > 2500, "expected opaque rounded tile interior");
        assert!(transparent > 100, "expected transparent rounded corners");
    }

    #[test]
    fn embedded_ico_has_four_expected_sizes() {
        let ico = include_bytes!("../assets/swarmforge.ico");
        assert!(ico.len() > 1024);
        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 4, 0]);
        let widths: Vec<u8> = (0..4).map(|i| ico[6 + i * 16]).collect();
        assert_eq!(widths, vec![16, 32, 48, 0]);
    }
}
