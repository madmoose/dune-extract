use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::PathBuf,
};

use crate::error::Error;
use crate::{bytes_ext::ReadBytesExt, unhsq::unhsq};

pub struct DatFile {
    reader: BufReader<File>,
    pub entries: Vec<DatEntry>,
}

#[derive(Debug)]
pub struct DatEntry {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

impl DatFile {
    pub fn open(path: &Option<PathBuf>) -> Result<DatFile, Error> {
        let path = match path {
            Some(p) if p.is_dir() => {
                let mut p = p.clone();
                p.push("DUNE.DAT");
                p
            }
            Some(p) => p.clone(),
            None => "DUNE.DAT".into(),
        };

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let entry_count = reader.read_le_u16()? as usize;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let name = reader.read_fixed_str(16)?;
            let size = reader.read_le_u32()? as usize;
            let offset = reader.read_le_u32()? as usize;
            _ = reader.read_u8();

            if name.is_empty() {
                break;
            }

            entries.push(DatEntry { name, size, offset });
        }

        Ok(DatFile { reader, entries })
    }

    pub fn read_raw(&mut self, name: &str) -> Result<Vec<u8>, Error> {
        let entry = self
            .entries
            .iter()
            .find(|&e| e.name == name)
            .ok_or(Error::EntryNotFound)?;

        self.reader
            .seek(std::io::SeekFrom::Start(entry.offset as u64))?;

        let mut data = vec![0; entry.size];
        self.reader.read_exact(data.as_mut_slice())?;

        Ok(data)
    }

    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, Error> {
        let data = self.read_raw(name)?;

        if !is_compressed(&data) {
            return Ok(data);
        }

        let mut reader = Cursor::new(&data);
        let unpacked_length = reader.read_le_u16()?;
        _ = reader.read_u8();
        let packed_length = reader.read_le_u16()?;
        _ = reader.read_u8();

        if packed_length as usize != data.len() {
            println!("Packed length does not match resource size");
            return Ok(data);
        }

        let mut unpacked_data = vec![0; unpacked_length as usize];

        unhsq(&data[6..], &mut unpacked_data);
        Ok(unpacked_data)
    }
}

fn is_compressed(header: &[u8]) -> bool {
    if header.len() < 6 {
        return false;
    }

    let checksum: u8 = header.iter().take(6).fold(0, |acc, &x| acc.wrapping_add(x));

    checksum == 0xab && header[2] == 0
}
