use std::{
    fs::File,
    io::{BufReader, Read, Seek},
    path::PathBuf,
};

use crate::bytes_ext::ReadBytesExt;

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
    pub fn open(path: &Option<PathBuf>) -> Option<DatFile> {
        let path = match path {
            Some(p) if p.is_dir() => {
                let mut p = p.clone();
                p.push("DUNE.DAT");
                p
            }
            Some(p) => p.clone(),
            None => "DUNE.DAT".into(),
        };

        let f = File::open(&path);
        let f = match f {
            Ok(f) => f,
            Err(_) => {
                eprintln!("Unable to open file `{}`", path.display());
                return None;
            }
        };

        let mut reader = BufReader::new(f);

        let entry_count = reader.read_le_u16().expect("Error reading dat-file") as usize;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let name = reader.read_fixed_str(16).expect("Error reading dat-file");
            let size = reader.read_le_u32().expect("Error reading dat-file") as usize;
            let offset = reader.read_le_u32().expect("Error reading dat-file") as usize;
            _ = reader.read_u8();

            if name.is_empty() {
                break;
            }

            entries.push(DatEntry { name, size, offset });
        }

        Some(DatFile { reader, entries })
    }

    pub fn read(&mut self, name: &str) -> Option<Vec<u8>> {
        let entry = self.entries.iter().find(|&e| e.name == name)?;

        self.reader
            .seek(std::io::SeekFrom::Start(entry.offset as u64))
            .expect("Failed to seek in dat-file");

        let mut data = vec![0; entry.size];
        self.reader
            .read_exact(data.as_mut_slice())
            .expect("Failed to read entry in dat-file");

        Some(data)
    }
}
