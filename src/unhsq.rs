use std::io::Cursor;

use crate::bytes_ext::ReadBytesExt;

struct Reader<'a> {
    queue: u16,
    r: Cursor<&'a [u8]>,
}

impl Reader<'_> {
    pub fn read_bit(&mut self) -> bool {
        let mut queue = self.queue;
        let mut bit = (queue & 1) == 1;
        queue >>= 1;
        if queue == 0 {
            queue = self.r.read_le_u16().unwrap();
            bit = (queue & 1) == 1;
            queue = 0x8000 | (queue >> 1);
        }
        self.queue = queue;
        bit
    }
    pub fn read_u8(&mut self) -> u8 {
        self.r.read_u8().unwrap()
    }
    pub fn read_le_u16(&mut self) -> u16 {
        self.r.read_le_u16().unwrap()
    }
}

pub fn unhsq(r: &[u8], w: &mut [u8]) {
    let mut r = Reader {
        queue: 0,
        r: Cursor::new(r),
    };
    let mut w_ofs: u16 = 0;

    loop {
        if r.read_bit() {
            w[w_ofs as usize] = r.read_u8();
            w_ofs += 1;
        } else {
            let mut count: u16;
            let offset: u16;
            if r.read_bit() {
                let word = r.read_le_u16();
                count = word & 7;
                offset = 8192 - (word >> 3);
                if count == 0 {
                    count = r.read_u8() as u16;
                }
                if count == 0 {
                    break;
                }
            } else {
                let b0 = r.read_bit() as u16;
                let b1 = r.read_bit() as u16;

                count = 2 * b0 + b1;
                offset = 256 - (r.read_u8() as u16);
            }

            for _ in 0..count + 2 {
                w[w_ofs as usize] = w[(w_ofs - offset) as usize];
                w_ofs += 1;
            }
        }
    }
}
