#![allow(clippy::identity_op)]

mod bytes_ext;
mod dat_file;
mod error;
mod sprite;
mod unhsq;

use std::{
    fs::{self, File},
    io::{self, BufWriter, Cursor, Read, Write},
    path::{Path, PathBuf},
    slice,
};

use clap::{Parser, Subcommand};
use itertools::Itertools;

use crate::{
    bytes_ext::{ReadBytesExt, WriteBytesExt},
    dat_file::DatFile,
    error::Error,
};

#[derive(Debug, Parser)]
#[command(name = "dune-extract")]
struct Cli {
    #[arg(long)]
    dat_path: Option<PathBuf>,
    #[arg(long, default_value = "dump")]
    out_path: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List the contents of DUNE.DAT
    List,
    /// Decompress RLE-compressed save file
    DecompressSav { file_name: String },
    /// Extracts all resource from DUNE.DAT, decompressing if needed
    ExtractAll,
    /// Extracts a resource from DUNE.DAT without decompressing
    ExtractRaw { entry_name: String },
    /// Extracts a resource from DUNE.DAT, decompressing if needed
    Extract { entry_name: String },
    /// Extracts sprite resources from a sprite sheet
    ExtractSprites { entry_name: String },
    /// Extracts font resource
    ExtractFont { entry_name: String },
}

fn create_file_for_entry(path: &Path, entry_name: &str) -> io::Result<File> {
    let path = path.join(entry_name.split('\\').collect::<PathBuf>());
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    File::create(path)
}

fn list(dat_file: &mut DatFile) {
    println!("+------------------+------------+------------+");
    println!("| name             |     offset |       size |");
    println!("+------------------+------------+------------+");
    for e in dat_file.entries.iter() {
        println!("| {:16} | {:-10} | {:-10} |", e.name, e.offset, e.size);
    }
    println!("+------------------+------------+------------+");
}

fn decompress_sav(file_name: &str) -> Result<(), Error> {
    let mut file = File::open(file_name)?;

    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    let data_len = data.len();

    let mut r = Cursor::new(data);
    let mut w = Vec::<u8>::new();

    let unk0 = r.read_le_u16()?;
    let rle_byte = r.read_le_u16()? as u8;
    // The length includes the rle-word and itself but not the first word
    let length = r.read_le_u16()? as usize;

    if length != data_len - 2 {
        println!(
            "`{}` is not a Dune save file - invalid length in header.",
            file_name
        );
        return Ok(());
    }

    w.write_le_u16(unk0)?;

    while let Ok(c) = r.read_u8() {
        if c == rle_byte {
            let cnt = r.read_u8()?;
            let val = r.read_u8()?;
            for _ in 0..cnt {
                w.write_all(slice::from_ref(&val))?;
            }
        } else {
            w.write_all(slice::from_ref(&c))?;
        }
    }

    let out_file_name: String = file_name
        .strip_suffix(".SAV")
        .unwrap_or(file_name)
        .to_owned()
        + ".BIN";

    let mut out_file = File::create(&out_file_name)?;
    out_file.write_all(&w)?;

    println!("Decompressed `{}` to `{}`", file_name, out_file_name);

    Ok(())
}

fn extract_all(path: &Path, dat_file: &mut DatFile) -> Result<(), Error> {
    let entry_names = dat_file
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect::<Vec<_>>();
    for name in entry_names.iter() {
        extract(path, dat_file, name)?;
    }
    Ok(())
}

fn extract_raw(path: &Path, dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    let data = dat_file.read_raw(entry_name)?;

    let mut f = create_file_for_entry(path, entry_name)?;
    f.write_all(data.as_slice())?;

    Ok(())
}

fn extract(path: &Path, dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    println!("Extracting `{}`", entry_name);

    let data = dat_file.read(entry_name).expect("Entry not found");

    let mut f = if let Some(prefix) = entry_name.strip_suffix(".HSQ") {
        let new_entry_name = prefix.to_owned() + ".BIN";
        create_file_for_entry(path, &new_entry_name)?
    } else {
        create_file_for_entry(path, entry_name)?
    };

    f.write_all(data.as_slice())?;

    Ok(())
}

fn extract_sprites(dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    println!("Extracting sprites from `{}`", entry_name);

    let data = dat_file.read(entry_name)?;
    let mut r = Cursor::new(data.as_slice());

    let toc_position = r.read_le_u16()?;
    r.set_position(toc_position as u64);

    let first_resource_offset = r.read_le_u16()?;
    let sub_resource_count = first_resource_offset / 2;

    if sub_resource_count == 0 || sub_resource_count > 1000 {
        panic!("Not a sprite sheet");
    }

    let mut sub_resource_offsets = Vec::with_capacity(sub_resource_count as usize);
    sub_resource_offsets.push(first_resource_offset);

    for _ in 1..sub_resource_count {
        sub_resource_offsets.push(r.read_le_u16()?);
    }

    for &offset in &sub_resource_offsets {
        if offset as usize >= data.len() {
            panic!("invalid toc, offset too large");
        }
    }

    // Validate that resource offsets are sequential
    for (a, b) in sub_resource_offsets.iter().tuple_windows() {
        if a >= b {
            panic!("invalid toc, non-sequential offsets");
        }
    }

    let mut pal = vec![0u8; 768];

    for i in 0..256 {
        pal[3 * i + 0] = i as u8;
        pal[3 * i + 1] = i as u8;
        pal[3 * i + 2] = i as u8;
    }

    if toc_position > 2 {
        r.set_position(2);
        loop {
            let mut v: u16;
            loop {
                v = r.read_le_u16().unwrap();
                if v != 256 {
                    break;
                }
                r.set_position(r.position() + 3);
            }
            if v == 0xffff {
                break;
            }

            let mut count = (v >> 8) & 0xff;
            let offset = v & 0xff;

            if count == 0 {
                count = 256;
            }

            for i in 0..3 * count {
                pal[(3 * offset + i) as usize] = r.read_u8()?;
            }
        }
    }

    let file_stem = Path::new(entry_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "SPRITE".into());

    for (i, &offset) in sub_resource_offsets.iter().enumerate() {
        r.set_position((toc_position as u64) + (offset as u64));

        let w0 = r.read_le_u16()?;
        let w1 = r.read_le_u16()?;

        let flags = ((w0 & 0xff00) >> 8) as u8;
        let width = (w0 & 0x7fff) as usize;
        let height = (w1 & 0x00ff) as usize;
        let pal_offset = ((w1 & 0xff00) >> 8) as u8;

        if !(1..=320).contains(&width) || !(1..=200).contains(&height) {
            continue;
        }

        let mut image_data = vec![0u8; width * height * 4];

        let filename = format!("{}-{:02}.png", file_stem, i);
        let path = Path::new(&filename);
        let file = File::create(path)?;
        let w = &mut BufWriter::new(file);

        let mut encoder = png::Encoder::new(w, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let is_rle_compressed = flags & 0x80 != 0;

        if pal_offset < 254 {
            if !is_rle_compressed {
                sprite::draw_4bpp(
                    image_data.as_mut_slice(),
                    &mut r,
                    width,
                    height,
                    &pal,
                    pal_offset,
                )?;
            } else {
                sprite::draw_4bpp_rle(
                    image_data.as_mut_slice(),
                    &mut r,
                    width,
                    height,
                    &pal,
                    pal_offset,
                )?;
            }
        } else if !is_rle_compressed {
            sprite::draw_8bpp(
                image_data.as_mut_slice(),
                &mut r,
                width,
                height,
                &pal,
                pal_offset,
            )?;
        } else {
            sprite::draw_8bpp_rle(
                image_data.as_mut_slice(),
                &mut r,
                width,
                height,
                &pal,
                pal_offset,
            )?;
        }

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image_data)?;
    }
    Ok(())
}

fn extract_font(dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    let cw = 8;
    let ch1 = 9;
    let ch2 = 7;
    let width = cw * 16;
    let height = ch1 * 8 + ch2 * 8;

    let mut image_data = vec![0u8; width * height * 4];

    let data = dat_file.read(entry_name)?;
    let mut r = Cursor::new(data.as_slice());

    let mut ws = [0; 256];
    for w in &mut ws {
        *w = r.read_u8()? as i32;
    }

    for al in 0..128 {
        r.set_position((0x100 + ch1 * al) as u64);

        let x = cw * (al % 16);
        let y = ch1 * (al / 16);

        for dy in 0..ch1 {
            let bs = r.read_u8()?;
            for dx in 0..cw {
                if (bs << dx) & 0x80 == 0x80 {
                    image_data[4 * ((y + dy) * width + (x + dx)) + 0] = 255;
                    image_data[4 * ((y + dy) * width + (x + dx)) + 1] = 255;
                    image_data[4 * ((y + dy) * width + (x + dx)) + 2] = 255;
                    image_data[4 * ((y + dy) * width + (x + dx)) + 3] = 255;
                }
            }
        }
    }
    for al in 0..128 {
        r.set_position((0x100 + 0x480 + ch2 * al) as u64);

        let x = cw * (al % 16);
        let y = ch2 * (al / 16) + (ch1 * 8);

        for dy in 0..ch2 {
            let bs = r.read_u8()?;
            for dx in 0..cw {
                if (bs << dx) & 0x80 == 0x80 {
                    image_data[4 * ((y + dy) * width + (x + dx)) + 0] = 255;
                    image_data[4 * ((y + dy) * width + (x + dx)) + 1] = 255;
                    image_data[4 * ((y + dy) * width + (x + dx)) + 2] = 255;
                    image_data[4 * ((y + dy) * width + (x + dx)) + 3] = 255;
                }
            }
        }
    }

    let file_stem = Path::new(entry_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap();

    let filename = format!("{}.png", file_stem);
    let path = Path::new(&filename);
    let file = File::create(path)?;
    let w = &mut BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image_data)?;

    println!("Glyph widths:\n{:?}", ws);

    Ok(())
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    let out_path = cli.out_path;
    let mut dat_file = DatFile::open(&cli.dat_path).expect("Failed to open DUNE.DAT");

    match &cli.command {
        Commands::List => list(&mut dat_file),
        Commands::DecompressSav { file_name } => {
            decompress_sav(file_name)?;
        }
        Commands::ExtractAll => {
            extract_all(&out_path, &mut dat_file)?;
        }
        Commands::ExtractRaw { entry_name } => {
            extract_raw(&out_path, &mut dat_file, entry_name)?;
        }
        Commands::Extract { entry_name } => {
            extract(&out_path, &mut dat_file, entry_name)?;
        }
        Commands::ExtractSprites { entry_name } => {
            extract_sprites(&mut dat_file, entry_name)?;
        }
        Commands::ExtractFont { entry_name } => {
            extract_font(&mut dat_file, entry_name)?;
        }
    }
    Ok(())
}
