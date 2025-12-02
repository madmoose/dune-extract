use std::{fs::File, io::BufWriter, path::Path};

use crate::pal::Pal;

pub struct Frame {
    data: Vec<u8>,
    width: usize,
    height: usize,
}

impl Frame {
    pub fn new(width: usize, height: usize) -> Self {
        let data = vec![0u8; width * height];
        Self {
            data,
            width,
            height,
        }
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub fn mut_data(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn clear(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.data[y * self.width + x] = 0;
            }
        }
    }

    pub fn write_pixel(&mut self, x: usize, y: usize, c: u8) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = c;
        }
    }

    pub fn write_png(&self, filename: &str, pal: &Pal) -> std::io::Result<()> {
        let path = Path::new(&filename);
        let file = File::create(path)?;
        let w = &mut BufWriter::new(file);

        fn scale_6bit_to_8bit(c: u8) -> u8 {
            (255 * (c as u16) / 63) as u8
        }

        let expanded_width = 5 * self.width();
        let expanded_height = 6 * self.height();
        let mut rgba_data = vec![0u8; expanded_width * expanded_height * 4];

        for y in 0..expanded_height {
            for x in 0..expanded_width {
                let c = self.data[(y / 6) * self.width + (x / 5)] as usize;
                let rgb = pal.get(c);
                rgba_data[4 * (y * expanded_width + x) + 0] = scale_6bit_to_8bit(rgb.0);
                rgba_data[4 * (y * expanded_width + x) + 1] = scale_6bit_to_8bit(rgb.1);
                rgba_data[4 * (y * expanded_width + x) + 2] = scale_6bit_to_8bit(rgb.2);
                rgba_data[4 * (y * expanded_width + x) + 3] = 255;
            }
        }

        let mut encoder = png::Encoder::new(w, expanded_width as u32, expanded_height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&rgba_data)?;

        Ok(())
    }
}
