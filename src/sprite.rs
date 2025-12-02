#![allow(clippy::too_many_arguments)]

use std::io::{Cursor, Seek};

use crate::{
    bytes_ext::{ReadBytesExt, WriteBytesExt},
    frame::Frame,
    pal::Pal,
};

pub struct SpriteSheet<'a> {
    offsets: Vec<(usize, usize)>,
    data: &'a [u8],
}

impl<'a> SpriteSheet<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, std::io::Error> {
        let size = data.len();

        let toc_pos = u16::from_le_bytes(data[0..2].try_into().unwrap()) as usize;

        let mut toc = Cursor::new(&data[toc_pos..]);

        let sprite_0_pos = toc.read_le_u16()? as usize;
        let sprite_count = sprite_0_pos / 2;

        let mut offsets = Vec::with_capacity(sprite_count);
        let mut prev_pos = sprite_0_pos;

        for _ in 1..sprite_count {
            let pos = toc.read_le_u16()? as usize;
            offsets.push((toc_pos + prev_pos, pos - prev_pos));
            prev_pos = pos;
        }
        offsets.push((toc_pos + prev_pos, size - toc_pos - prev_pos));

        Ok(SpriteSheet { offsets, data })
    }

    pub fn apply_palette_update(&self, pal: &mut Pal) -> Result<(), std::io::Error> {
        let mut r = Cursor::new(self.data);
        let toc_pos = r.read_le_u16()?;

        if toc_pos <= 2 {
            return Ok(());
        }

        loop {
            let index = r.read_u8()? as usize;
            let mut count = r.read_u8()? as usize;

            if index == 1 && count == 0 {
                r.seek_relative(3)?;
                continue;
            }
            if index == 0xff && count == 0xff {
                break;
            }
            if count == 0 {
                count = 256;
            }

            for i in 0..count {
                let cr = r.read_u8()?;
                let cg = r.read_u8()?;
                let cb = r.read_u8()?;

                pal.set(index + i, (cr, cg, cb));
            }
        }

        Ok(())
    }

    pub fn get_sprite(&'a self, id: u16) -> Option<Sprite<'a>> {
        let &(ofs, size) = self.offsets.get(id as usize)?;
        Some(Sprite::new_from_slice(
            id as usize,
            &self.data[ofs..ofs + size],
        ))
    }
}

#[derive(Debug)]
pub struct Sprite<'a> {
    id: usize,
    width: u16,
    height: u16,
    pal_offset: u8,
    rle: bool,
    // flip_x: bool,
    // flip_y: bool,
    // scale: u8,
    data: &'a [u8],
}

impl<'a> Sprite<'a> {
    pub fn new_from_slice(id: usize, data: &'a [u8]) -> Self {
        let w0 = u16::from_le_bytes(data[0..2].try_into().unwrap());
        let w1 = u16::from_le_bytes(data[2..4].try_into().unwrap());
        let data = &data[4..];

        let flags = (w0 & 0xfe00) >> 8;
        let width = w0 & 0x01ff;
        let pal_offset = ((w1 & 0xff00) >> 8) as u8;
        let height = w1 & 0x00ff;

        let rle = (flags & 0x80) != 0;
        let _flip_x = (flags & 0x40) != 0;
        let _flip_y = (flags & 0x20) != 0;
        let _scale = ((flags & 0x1c) >> 2) as u8;

        Sprite {
            id,
            width,
            height,
            pal_offset,
            rle,
            // flip_x,
            // flip_y,
            // scale,
            data,
        }
    }

    pub fn bpp(&self) -> usize {
        if self.pal_offset < 254 {
            4
        } else {
            8
        }
    }

    pub fn width(&self) -> usize {
        self.width as usize
    }

    pub fn height(&self) -> usize {
        self.height as usize
    }

    pub fn pitch(&self) -> usize {
        if self.bpp() == 8 {
            self.width()
        } else {
            2 * self.width().div_ceil(4)
        }
    }

    pub fn pal_offset(&self) -> u8 {
        self.pal_offset
    }

    pub fn set_pal_offset(&mut self, pal_offset: u8) {
        self.pal_offset = pal_offset;
    }

    pub fn rle(&self) -> bool {
        self.rle
    }

    pub fn data(&self) -> &[u8] {
        self.data
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        x: usize,
        y: usize,
        flip_x: bool,
        flip_y: bool,
        scale: u8,
        pal_offset: u8,
    ) -> std::io::Result<()> {
        if self.bpp() == 8 {
            if self.rle() {
                let src = self.unrle()?;
                self.draw_8bpp(&src, frame, x, y, flip_x, flip_y, scale, pal_offset);
            } else {
                let src = self.data();
                self.draw_8bpp(src, frame, x, y, flip_x, flip_y, scale, pal_offset);
            }

            // exit(0);
            // return Ok(());
        } else if self.rle() {
            let src = self.unrle()?;
            self.draw_4bb(&src, frame, x, y, flip_x, flip_y, scale, pal_offset);
        } else {
            let src = self.data();
            self.draw_4bb(src, frame, x, y, flip_x, flip_y, scale, pal_offset);
        }
        Ok(())
    }

    fn draw_4bb(
        &self,
        src: &[u8],
        frame: &mut Frame,
        x: usize,
        y: usize,
        flip_x: bool,
        flip_y: bool,
        _scale: u8,
        pal_offset: u8,
    ) {
        let dst_x = x;
        let dst_y = y;

        let pal_offset = if pal_offset != 0 {
            pal_offset
        } else {
            self.pal_offset()
        };

        let height = self.height();
        let width = self.width();
        let pitch = self.pitch();

        for y in 0..height {
            for x in 0..width {
                let mut c = src[y * pitch + x / 2];
                if x & 1 == 0 {
                    c &= 0xf;
                } else {
                    c >>= 4;
                }

                if c != 0 {
                    let x = dst_x + if flip_x { width - x - 1 } else { x };
                    let y = dst_y + if flip_y { height - y - 1 } else { y };

                    frame.write_pixel(x, y, c + pal_offset);
                }
            }
        }
    }

    fn draw_8bpp(
        &self,
        src: &[u8],
        frame: &mut Frame,
        x: usize,
        y: usize,
        flip_x: bool,
        flip_y: bool,
        _scale: u8,
        mode: u8,
    ) {
        let dst_x = x;
        let dst_y = y;
        let height = self.height();
        let width = self.width();
        let pitch = self.pitch();
        let mode = if mode != 0 { mode } else { self.pal_offset() };

        for y in 0..height {
            for x in 0..width {
                let c = src[y * pitch + x];
                if mode != 255 || c != 0 {
                    let x = dst_x + if flip_x { width - x - 1 } else { x };
                    let y = dst_y + if flip_y { height - y - 1 } else { y };
                    frame.write_pixel(x, y, c);
                }
            }
        }
    }

    fn unrle(&self) -> std::io::Result<Vec<u8>> {
        let pitch = self.pitch();
        let mut buf = vec![0u8; self.height() * pitch];

        let mut rle_src = Cursor::new(self.data());
        let mut rle_dst = Cursor::new(&mut buf);

        for _ in 0..self.height() {
            let mut x = 0;
            while x < pitch {
                let count;
                let cmd = rle_src.read_u8()?;
                if cmd & 0x80 != 0 {
                    count = 257 - (cmd as usize);
                    let value = rle_src.read_u8()?;
                    for _ in 0..count {
                        rle_dst.write_u8(value)?;
                    }
                } else {
                    count = (cmd as usize) + 1;
                    for _ in 0..count {
                        let value = rle_src.read_u8()?;
                        rle_dst.write_u8(value)?;
                    }
                }

                x += count;
            }
        }

        Ok(buf)
    }
}
