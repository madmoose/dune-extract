pub trait WriteBytesExt: std::io::Write {
    #[inline]
    fn write_le_u16(&mut self, v: u16) -> std::io::Result<()> {
        let buf = v.to_le_bytes();
        self.write_all(&buf)?;
        Ok(())
    }
}

impl<W: std::io::Write> WriteBytesExt for W {}

pub trait ReadBytesExt: std::io::Read {
    #[inline]
    fn read_u8(&mut self) -> std::io::Result<u8> {
        let mut buf = [0; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    #[inline]
    fn read_le_u16(&mut self) -> std::io::Result<u16> {
        let mut buf = [0; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    #[inline]
    fn read_le_u32(&mut self) -> std::io::Result<u32> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_fixed_str(&mut self, len: usize) -> std::io::Result<String> {
        let mut s = String::new();
        let mut bytes_read = 0;
        let mut found_end_of_string = false;

        while bytes_read < len {
            let c = self.read_u8()?;
            bytes_read += 1;

            if found_end_of_string {
            } else if c == 0 {
                found_end_of_string = true;
            } else {
                s.push(c as char);
            }
        }

        Ok(s)
    }
}

impl<R: std::io::Read> ReadBytesExt for R {}
