pub struct Pal([u8; 768]);

impl Pal {
    pub fn new() -> Self {
        let pal = [0u8; 768];
        Pal(pal)
    }

    pub fn new_from_slice(slice: &[u8; 768]) -> Self {
        Pal(*slice)
    }

    pub fn clear(&mut self) {
        for i in 0..256 {
            self.set(i, (0, 0, 0));
        }
    }

    pub fn get(&self, i: usize) -> (u8, u8, u8) {
        let r = self.0[3 * i + 0];
        let g = self.0[3 * i + 1];
        let b = self.0[3 * i + 2];

        (r, g, b)
    }

    pub fn set(&mut self, i: usize, rgb: (u8, u8, u8)) {
        self.0[3 * i + 0] = rgb.0;
        self.0[3 * i + 1] = rgb.1;
        self.0[3 * i + 2] = rgb.2;
    }

    pub fn as_slice(&self) -> &[u8; 768] {
        &self.0
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8; 768] {
        &mut self.0
    }
}
