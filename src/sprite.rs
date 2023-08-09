use std::io::{self, Cursor};

use crate::bytes_ext::ReadBytesExt;

fn scale_6bit_to_8bit(c: u8) -> u8 {
    (255 * (c as u16) / 63) as u8
}

fn write_pixel(dst: &mut [u8], w: usize, x: usize, y: usize, pal: &[u8], c: u8) {
    let c = c as usize;
    dst[4 * (y * w + x) + 0] = scale_6bit_to_8bit(pal[3 * c + 0]);
    dst[4 * (y * w + x) + 1] = scale_6bit_to_8bit(pal[3 * c + 1]);
    dst[4 * (y * w + x) + 2] = scale_6bit_to_8bit(pal[3 * c + 2]);
    dst[4 * (y * w + x) + 3] = 255;
}

pub fn draw_4bpp(dst: &mut [u8], src: &mut Cursor<&[u8]>, w: usize, h: usize, pal: &[u8], mode: u8) -> io::Result<()> {
    for y in 0..h {
        let mut line_remain = 4 * ((w + 3) / 4);
        let mut x = 0;
        while line_remain > 0 {
            let value = src.read_u8()?;
            let p1 = value & 0x0f;
            let p2 = value >> 4;

            if p1 != 0 && x < w {
                write_pixel(dst, w, x, y, pal, p1 + mode);
            }
            x += 1;

            if p2 != 0 && x < w {
                write_pixel(dst, w, x, y, pal, p2 + mode);
            }
            x += 1;

            line_remain -= 2;
        }
    }
	Ok(())
}

pub fn draw_4bpp_rle(
    dst: &mut [u8],
    src: &mut Cursor<&[u8]>,
    w: usize,
    h: usize,
    pal: &[u8],
    mode: u8,
)  -> io::Result<()> {
    for y in 0..h {
        let mut line_remain = 4 * ((w + 3) / 4);
        let mut x = 0;
        while line_remain > 0 {
            let cmd = src.read_u8()?;
            if cmd & 0x80 != 0 {
                let count = 257 - (cmd as u16);
                let value = src.read_u8()?;

                let p1 = value & 0x0f;
                let p2 = value >> 4;
                for _ in 0..count {
                    if p1 != 0 {
                        write_pixel(dst, w, x, y, pal, p1 + mode);
                    }
                    x += 1;
                    if p2 != 0 {
                        write_pixel(dst, w, x, y, pal, p2 + mode);
                    }
                    x += 1;
                }
                line_remain -= 2 * (count as usize);
            } else {
                let count = (cmd + 1) as u16;
                for _ in 0..count {
                    let value = src.read_u8()?;

                    let p1 = value & 0x0f;
                    let p2 = value >> 4;

                    if p1 != 0 {
                        write_pixel(dst, w, x, y, pal, p1 + mode);
                    }
                    x += 1;

                    if p2 != 0 {
                        write_pixel(dst, w, x, y, pal, p2 + mode);
                    }
                    x += 1;
                }
                line_remain -= 2 * (count as usize);
            }
        }
    }
	Ok(())
}

pub fn draw_8bpp(dst: &mut [u8], src: &mut Cursor<&[u8]>, w: usize, h: usize, pal: &[u8], mode: u8) -> io::Result<()> {
    for y in 0..h {
        for x in 0..w {
            let value = src.read_u8()?;
            if mode != 255 && value != 0 {
                write_pixel(dst, w, x, y, pal, value);
            }
        }
    }
	Ok(())
}

pub fn draw_8bpp_rle(
    dst: &mut [u8],
    src: &mut Cursor<&[u8]>,
    w: usize,
    h: usize,
    pal: &[u8],
    mode: u8,
)  -> io::Result<()> {
    for y in 0..h {
        let mut x = 0;

        while x < w {
            let cmd = src.read_u8()?;
            if cmd & 0x80 != 0 {
                let count = 257 - (cmd as u16);
                let value = src.read_u8()?;
                for _ in 0..count {
                    if (mode != 255 || value != 0) && x < w {
                        write_pixel(dst, w, x, y, pal, value);
                    }
                    x += 1;
                }
            } else {
                let count = (cmd + 1) as u16;
                for _ in 0..count {
                    let value = src.read_u8()?;
                    if (mode != 255 || value != 0) && x < w {
                        write_pixel(dst, w, x, y, pal, value);
                    }
                    x += 1;
                }
            }
        }
    }
	Ok(())
}
