mod bytes_ext;
mod dat_file;
mod error;
mod sprite;
mod unhsq;

use std::{
    ffi::OsStr,
    fs::File,
    io::{BufWriter, Cursor, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use itertools::Itertools;

use crate::{bytes_ext::ReadBytesExt, dat_file::DatFile, error::Error};

#[derive(Debug, Parser)]
#[command(name = "dune-extract")]
struct Cli {
    #[arg(long)]
    dat_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List the contents of DUNE.DAT
    List,
    /// Extracts all resource from DUNE.DAT, decompressing if needed
    ExtractAll,
    /// Extracts a resource from DUNE.DAT without decompressing
    #[command(arg_required_else_help = true)]
    ExtractRaw { entry_name: String },
    /// Extracts a resource from DUNE.DAT, decompressing if needed
    Extract { entry_name: String },
    /// Extracts sprite resources from a sprite sheet
    ExtractSprites { entry_name: String },
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

fn extract_all(dat_file: &mut DatFile) -> Result<(), Error> {
    let entry_names = dat_file
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect::<Vec<_>>();
    for name in entry_names.iter() {
        extract(dat_file, name)?;
    }
    Ok(())
}

fn extract_raw(dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    let data = dat_file.read_raw(entry_name)?;

    let mut f = File::create(entry_name)?;
    f.write_all(data.as_slice())?;

    Ok(())
}

fn extract(dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    println!("Extracting `{}`", entry_name);

    let data = dat_file.read(entry_name).expect("Entry not found");

    let mut path: PathBuf = entry_name.into();
    if path.extension() == Some(OsStr::new("HSQ")) {
        path.set_extension("BIN");
    }

    let mut f = File::create(&path)?;

    f.write_all(data.as_slice())?;

    println!("Extracted to `{}`", path.display());

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
        let ref mut w = BufWriter::new(file);

        let mut encoder = png::Encoder::new(w, width as u32, height as u32); // Width is 2 pixels and height is 1.
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
        } else {
            if !is_rle_compressed {
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
        }

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image_data)?;
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    let mut dat_file = DatFile::open(&cli.dat_path).expect("Failed to open DUNE.DAT");

    match &cli.command {
        Commands::List => list(&mut dat_file),
        Commands::ExtractAll => {
            extract_all(&mut dat_file)?;
        }
        Commands::ExtractRaw { entry_name } => {
            extract_raw(&mut dat_file, entry_name)?;
        }
        Commands::Extract { entry_name } => {
            extract(&mut dat_file, entry_name)?;
        }
        Commands::ExtractSprites { entry_name } => {
            extract_sprites(&mut dat_file, entry_name)?;
        }
    }
    Ok(())
}
