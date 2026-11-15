//! Pixel format conversions, 10-bit handling, and container orientation transforms.

use crate::types::{PixelFormat, Rotation, VideoFrame};

/// Converts planar YUV 4:2:0 (8-bit) to packed RGBA8 using standard BT.709 matrix.
pub fn yuv420p_to_rgba8(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: usize,
    height: usize,
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
) -> Vec<u8> {
    let mut rgba = vec![0u8; width * height * 4];

    for row in 0..height {
        let y_row_offset = row * y_stride;
        let uv_row_offset = (row / 2) * u_stride;
        let v_row_offset = (row / 2) * v_stride;
        let rgba_row_offset = row * width * 4;

        for col in 0..width {
            let y = y_plane[y_row_offset + col] as i32;
            let u = u_plane[uv_row_offset + (col / 2)] as i32 - 128;
            let v = v_plane[v_row_offset + (col / 2)] as i32 - 128;

            // BT.709 integer arithmetic (scaled by 1024)
            // R = Y + 1.5748 * V
            // G = Y - 0.1873 * U - 0.4681 * V
            // B = Y + 1.8556 * U
            let r = (y * 1024 + 1613 * v) / 1024;
            let g = (y * 1024 - 192 * u - 479 * v) / 1024;
            let b = (y * 1024 + 1900 * u) / 1024;

            let out_idx = rgba_row_offset + col * 4;
            rgba[out_idx] = r.clamp(0, 255) as u8;
            rgba[out_idx + 1] = g.clamp(0, 255) as u8;
            rgba[out_idx + 2] = b.clamp(0, 255) as u8;
            rgba[out_idx + 3] = 255;
        }
    }

    rgba
}

/// Converts planar YUV 4:2:0 (10-bit little-endian) to packed RGBA8.
/// Samples are 16-bit integers with 10 bits of precision (0..1023).
pub fn yuv420p10le_to_rgba8(
    y_plane: &[u16],
    u_plane: &[u16],
    v_plane: &[u16],
    width: usize,
    height: usize,
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
) -> Vec<u8> {
    let mut rgba = vec![0u8; width * height * 4];

    for row in 0..height {
        let y_row_offset = row * y_stride;
        let uv_row_offset = (row / 2) * u_stride;
        let v_row_offset = (row / 2) * v_stride;
        let rgba_row_offset = row * width * 4;

        for col in 0..width {
            // Scale from 10-bit (0..1023) down to 8-bit equivalent for RGB matrix
            let y = (y_plane[y_row_offset + col] >> 2) as i32;
            let u = (u_plane[uv_row_offset + (col / 2)] >> 2) as i32 - 128;
            let v = (v_plane[v_row_offset + (col / 2)] >> 2) as i32 - 128;

            let r = (y * 1024 + 1613 * v) / 1024;
            let g = (y * 1024 - 192 * u - 479 * v) / 1024;
            let b = (y * 1024 + 1900 * u) / 1024;

            let out_idx = rgba_row_offset + col * 4;
            rgba[out_idx] = r.clamp(0, 255) as u8;
            rgba[out_idx + 1] = g.clamp(0, 255) as u8;
            rgba[out_idx + 2] = b.clamp(0, 255) as u8;
            rgba[out_idx + 3] = 255;
        }
    }

    rgba
}

/// Rotates an RGBA8 buffer clockwise according to the specified `Rotation`.
/// Returns `(rotated_bytes, new_width, new_height)`.
pub fn rotate_rgba8(
    src: &[u8],
    width: u32,
    height: u32,
    rotation: Rotation,
) -> (Vec<u8>, u32, u32) {
    let w = width as usize;
    let h = height as usize;

    match rotation {
        Rotation::None => (src.to_vec(), width, height),

        Rotation::Rotate90 => {
            let new_w = height;
            let new_h = width;
            let mut out = vec![0u8; (new_w * new_h * 4) as usize];

            for y in 0..h {
                for x in 0..w {
                    let src_idx = (y * w + x) * 4;
                    // In 90 deg clockwise: (x, y) -> (h - 1 - y, x)
                    let new_x = h - 1 - y;
                    let new_y = x;
                    let dst_idx = (new_y * (new_w as usize) + new_x) * 4;

                    out[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
                }
            }

            (out, new_w, new_h)
        }

        Rotation::Rotate180 => {
            let mut out = vec![0u8; (width * height * 4) as usize];

            for y in 0..h {
                for x in 0..w {
                    let src_idx = (y * w + x) * 4;
                    let new_x = w - 1 - x;
                    let new_y = h - 1 - y;
                    let dst_idx = (new_y * w + new_x) * 4;

                    out[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
                }
            }

            (out, width, height)
        }

        Rotation::Rotate270 => {
            let new_w = height;
            let new_h = width;
            let mut out = vec![0u8; (new_w * new_h * 4) as usize];

            for y in 0..h {
                for x in 0..w {
                    let src_idx = (y * w + x) * 4;
                    // In 270 deg clockwise: (x, y) -> (y, w - 1 - x)
                    let new_x = y;
                    let new_y = w - 1 - x;
                    let dst_idx = (new_y * (new_w as usize) + new_x) * 4;

                    out[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
                }
            }

            (out, new_w, new_h)
        }
    }
}

impl VideoFrame {
    /// Extracts or converts this frame to packed 8-bit RGBA pixels,
    /// applying container rotation if any is specified.
    pub fn to_oriented_rgba8(&self) -> (Vec<u8>, u32, u32) {
        let base_rgba = match self.format {
            PixelFormat::Rgba8 => self.data.clone(),

            PixelFormat::Rgb24 => {
                let count = (self.width * self.height) as usize;
                let mut rgba = Vec::with_capacity(count * 4);
                for i in 0..count {
                    rgba.push(self.data[i * 3]);
                    rgba.push(self.data[i * 3 + 1]);
                    rgba.push(self.data[i * 3 + 2]);
                    rgba.push(255);
                }
                rgba
            }

            PixelFormat::Yuv420p => {
                let w = self.width as usize;
                let h = self.height as usize;
                let y_stride = if self.linesize[0] > 0 { self.linesize[0] } else { w };
                let u_stride = if self.linesize[1] > 0 { self.linesize[1] } else { w / 2 };
                let v_stride = if self.linesize[2] > 0 { self.linesize[2] } else { w / 2 };

                let y_size = y_stride * h;
                let u_size = u_stride * (h / 2);
                let v_size = v_stride * (h / 2);

                if self.data.len() >= y_size + u_size + v_size {
                    let y_plane = &self.data[..y_size];
                    let u_plane = &self.data[y_size..y_size + u_size];
                    let v_plane = &self.data[y_size + u_size..y_size + u_size + v_size];
                    yuv420p_to_rgba8(y_plane, u_plane, v_plane, w, h, y_stride, u_stride, v_stride)
                } else {
                    vec![0u8; w * h * 4]
                }
            }

            PixelFormat::Yuv420p10le => {
                let w = self.width as usize;
                let h = self.height as usize;
                let y_stride = if self.linesize[0] > 0 { self.linesize[0] / 2 } else { w };
                let u_stride = if self.linesize[1] > 0 { self.linesize[1] / 2 } else { w / 2 };
                let v_stride = if self.linesize[2] > 0 { self.linesize[2] / 2 } else { w / 2 };

                // Interpret u8 bytes as u16 little-endian
                let u16_data: Vec<u16> = self
                    .data
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();

                let y_size = y_stride * h;
                let u_size = u_stride * (h / 2);
                let v_size = v_stride * (h / 2);

                if u16_data.len() >= y_size + u_size + v_size {
                    let y_plane = &u16_data[..y_size];
                    let u_plane = &u16_data[y_size..y_size + u_size];
                    let v_plane = &u16_data[y_size + u_size..y_size + u_size + v_size];
                    yuv420p10le_to_rgba8(y_plane, u_plane, v_plane, w, h, y_stride, u_stride, v_stride)
                } else {
                    vec![0u8; w * h * 4]
                }
            }

            PixelFormat::Nv12 => {
                // NV12 fallback: convert semi-planar to RGBA
                let w = self.width as usize;
                let h = self.height as usize;
                let mut rgba = vec![0u8; w * h * 4];
                let y_stride = if self.linesize[0] > 0 { self.linesize[0] } else { w };
                let uv_stride = if self.linesize[1] > 0 { self.linesize[1] } else { w };
                let uv_offset = y_stride * h;

                for row in 0..h {
                    for col in 0..w {
                        let y = self.data.get(row * y_stride + col).copied().unwrap_or(0) as i32;
                        let uv_idx = uv_offset + (row / 2) * uv_stride + (col / 2) * 2;
                        let u = self.data.get(uv_idx).copied().unwrap_or(128) as i32 - 128;
                        let v = self.data.get(uv_idx + 1).copied().unwrap_or(128) as i32 - 128;

                        let r = (y * 1024 + 1613 * v) / 1024;
                        let g = (y * 1024 - 192 * u - 479 * v) / 1024;
                        let b = (y * 1024 + 1900 * u) / 1024;

                        let out_idx = (row * w + col) * 4;
                        rgba[out_idx] = r.clamp(0, 255) as u8;
                        rgba[out_idx + 1] = g.clamp(0, 255) as u8;
                        rgba[out_idx + 2] = b.clamp(0, 255) as u8;
                        rgba[out_idx + 3] = 255;
                    }
                }
                rgba
            }
        };

        rotate_rgba8(&base_rgba, self.width, self.height, self.rotation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotation_transposition() {
        let pixels = vec![
            10, 20, 30, 255,   40, 50, 60, 255,
            70, 80, 90, 255,  100, 110, 120, 255,
        ]; // 2x2 image

        let (r0, w0, h0) = rotate_rgba8(&pixels, 2, 2, Rotation::None);
        assert_eq!(pixels, r0);
        assert_eq!((w0, h0), (2, 2));

        let (r90, w90, h90) = rotate_rgba8(&pixels, 2, 2, Rotation::Rotate90);
        assert_eq!((w90, h90), (2, 2));
        // Top-left pixel of rotated 90 deg clockwise should be old bottom-left (70, 80, 90)
        assert_eq!(&r90[0..4], &[70, 80, 90, 255]);

        let (r180, _, _) = rotate_rgba8(&pixels, 2, 2, Rotation::Rotate180);
        // Top-left of 180 deg should be old bottom-right (100, 110, 120)
        assert_eq!(&r180[0..4], &[100, 110, 120, 255]);
    }

    #[test]
    fn test_10bit_conversion_produces_valid_rgba() {
        let width = 4;
        let height = 4;
        // Grey color in 10-bit: Y = 512, U = 512, V = 512
        let y_plane = vec![512u16; width * height];
        let u_plane = vec![512u16; (width / 2) * (height / 2)];
        let v_plane = vec![512u16; (width / 2) * (height / 2)];

        let rgba = yuv420p10le_to_rgba8(
            &y_plane,
            &u_plane,
            &v_plane,
            width,
            height,
            width,
            width / 2,
            width / 2,
        );

        assert_eq!(rgba.len(), width * height * 4);
        // Mid-grey 512 in 10-bit is 128 in 8-bit
        for pixel in rgba.chunks(4) {
            assert!((pixel[0] as i32 - 128).abs() <= 2);
            assert!((pixel[1] as i32 - 128).abs() <= 2);
            assert!((pixel[2] as i32 - 128).abs() <= 2);
            assert_eq!(pixel[3], 255);
        }
    }
}
