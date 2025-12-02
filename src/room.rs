use std::io::Cursor;

use itertools::Itertools;

use crate::{bytes_ext::ReadBytesExt, frame::Frame, sprite::SpriteSheet};

pub struct RoomSheet {
    rooms: Vec<Room>,
}

pub struct Room {
    position_marker_count: u8,
    parts: Vec<Part>,
}

enum Part {
    Sprite {
        id: u16,
        x: u16,
        y: u8,
        flip_x: bool,
        flip_y: bool,
        scale: u8,
        pal_offset: u8,
    },
    Character {
        x: u16,
        y: u8,
        pal_offset: u8,
    },
    Polygon {
        right_vertices: Vec<(u16, u16)>,
        left_vertices: Vec<(u16, u16)>,
        h_gradient: i16,
        v_gradient: i16,
        color: u8,
    },
    Line {
        p0: (u16, u16),
        p1: (u16, u16),
        color: u8,
        dither: u16,
    },
}

#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    FormatError(&'static str),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error)
    }
}

impl RoomSheet {
    pub fn new(data: &[u8]) -> Result<Self, Error> {
        let mut r = Cursor::new(data);

        let room_0_ofs = r.read_le_u16()?;
        let room_count = room_0_ofs / 2;
        if room_count == 0 {
            return Result::Err(Error::FormatError("invalid room count"));
        }

        let mut room_offsets = Vec::with_capacity(room_count.into());
        room_offsets.push(room_0_ofs);

        for _ in 1..room_count {
            room_offsets.push(r.read_le_u16()?);
        }

        let mut rooms = Vec::with_capacity(room_count.into());
        for ofs in room_offsets {
            r.set_position(ofs.into());

            let position_marker_count = r.read_u8()?;
            let mut parts = Vec::new();

            loop {
                let cmd = r.read_le_u16()?;
                if cmd == 0xffff {
                    break;
                }

                if (cmd & 0x8000) == 0 {
                    let x = (r.read_u8()? as u16) + if (cmd & 0x0200) != 0 { 256 } else { 0 };
                    let y = r.read_u8()?;
                    let pal_offset = r.read_u8()?;

                    if (cmd & 0x1ff) == 1 {
                        parts.push(Part::Character { x, y, pal_offset });
                    } else {
                        parts.push(Part::Sprite {
                            id: (cmd & 0x1ff) - 1,
                            x,
                            y,
                            flip_x: cmd & 0x4000 != 0,
                            flip_y: cmd & 0x2000 != 0,
                            scale: ((cmd >> 10) & 7) as u8,
                            pal_offset,
                        });
                    }
                } else if (cmd & 0x4000) == 0 {
                    // Polygon
                    let h_gradient = 16 * (r.read_i8()? as i16);
                    let v_gradient = 16 * (r.read_i8()? as i16);

                    let start_x = r.read_le_u16()?;
                    let start_y = r.read_le_u16()?;

                    let mut x;
                    let mut y;

                    let mut right_vertices = Vec::new();
                    let mut left_vertices = Vec::new();

                    right_vertices.push((start_x, start_y));

                    loop {
                        x = r.read_le_u16()?;
                        y = r.read_le_u16()?;

                        right_vertices.push((x & 0x3fff, y));

                        if x & 0x4000 != 0 {
                            break;
                        }
                    }

                    if x & 0x8000 == 0 {
                        loop {
                            x = r.read_le_u16()?;
                            y = r.read_le_u16()?;

                            left_vertices.push((x & 0x3fff, y));

                            if x & 0x8000 != 0 {
                                break;
                            }
                        }
                    }

                    parts.push(Part::Polygon {
                        right_vertices,
                        left_vertices,
                        h_gradient,
                        v_gradient,
                        color: (cmd & 0xff) as u8,
                    })
                } else {
                    // Line
                    let p0 = (r.read_le_u16()?, r.read_le_u16()?);
                    let p1 = (r.read_le_u16()?, r.read_le_u16()?);
                    parts.push(Part::Line {
                        p0,
                        p1,
                        color: (cmd & 0xff) as u8,
                        dither: 0xffffu16,
                    });
                }
            }
            rooms.push(Room {
                position_marker_count,
                parts,
            });
        }
        Ok(RoomSheet { rooms })
    }

    pub fn at(&self, room: usize) -> Option<&Room> {
        self.rooms.get(room)
    }
}

impl Room {
    pub fn draw(&self, frame: &mut Frame, sprite_sheet: &SpriteSheet) {
        for part in &self.parts {
            match part {
                Part::Sprite {
                    id,
                    x,
                    y,
                    flip_x,
                    flip_y,
                    scale,
                    pal_offset,
                } => {
                    let Some(sprite) = sprite_sheet.get_sprite(*id) else {
                        continue;
                    };
                    sprite
                        .draw(
                            frame,
                            *x as usize,
                            *y as usize,
                            *flip_x,
                            *flip_y,
                            *scale,
                            *pal_offset,
                        )
                        .unwrap();
                }
                Part::Character {
                    x: _,
                    y: _,
                    pal_offset: _,
                } => {}
                Part::Polygon {
                    right_vertices,
                    left_vertices,
                    h_gradient,
                    v_gradient,
                    color,
                } => {
                    self.draw_polygon(
                        frame,
                        right_vertices,
                        left_vertices,
                        *h_gradient,
                        *v_gradient,
                        *color,
                    );
                }
                Part::Line {
                    p0,
                    p1,
                    color,
                    dither,
                } => {
                    self.draw_line(frame, *p0, *p1, *color, *dither);
                }
            }
        }
    }

    fn draw_line(&self, frame: &mut Frame, p0: (u16, u16), p1: (u16, u16), color: u8, dither: u16) {
        let mut dither = dither;

        bresenham_line(p0, p1, |x, y| {
            dither = dither.rotate_left(1);
            if dither & 1 != 0 {
                frame.write_pixel(x, y, color);
            }
        });
    }

    fn draw_polygon(
        &self,
        frame: &mut Frame,
        right_vertices: &[(u16, u16)],
        left_vertices: &[(u16, u16)],
        _h_gradient: i16,
        _v_gradient: i16,
        color: u8,
    ) {
        println!("right_vertices = {:?}", right_vertices);
        println!("left_vertices = {:?}\n", left_vertices);

        let mut right_side = [0u16; 200];
        let mut left_side = [0u16; 200];

        right_vertices.iter().tuples().for_each(|(&p0, &p1)| {
            bresenham_line(p0, p1, |x, y| {
                right_side[y] = x as u16;
            });
        });

        if left_vertices.is_empty() {
            bresenham_line(
                *right_vertices.first().unwrap(),
                *right_vertices.last().unwrap(),
                |x, y| {
                    left_side[y] = x as u16;
                },
            );
        } else {
            bresenham_line(
                *right_vertices.first().unwrap(),
                *left_vertices.first().unwrap(),
                |x, y| {
                    left_side[y] = x as u16;
                },
            );

            left_vertices.iter().tuples().for_each(|(&p0, &p1)| {
                bresenham_line(p0, p1, |x, y| {
                    left_side[y] = x as u16;
                });
            });

            bresenham_line(
                *right_vertices.last().unwrap(),
                *left_vertices.last().unwrap(),
                |x, y| {
                    left_side[y] = x as u16;
                },
            );
        }

        for (y, (x0, x1)) in right_side
            .into_iter()
            .zip(left_side.into_iter())
            .enumerate()
        {
            for x in x0..x1 {
                frame.write_pixel(x as usize, y, color);
            }
        }
    }
}

fn bresenham_line<F>(p0: (u16, u16), p1: (u16, u16), mut f: F)
where
    F: FnMut(usize, usize),
{
    let mut x0 = p0.0 as i16;
    let mut y0 = p0.1 as i16;
    let x1 = p1.0 as i16;
    let y1 = p1.1 as i16;

    let dx = i16::abs(x1 - x0);
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -i16::abs(y1 - y0);
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        f(x0 as usize, y0 as usize);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * error;
        if e2 >= dy {
            error += dy;
            x0 += sx;
        }
        if e2 <= dx {
            error += dx;
            y0 += sy;
        }
    }
}
