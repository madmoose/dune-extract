#![feature(cursor_split)]
#![allow(clippy::identity_op)]
#![allow(dead_code)]

mod bytes_ext;
mod dat_file;
mod error;
mod frame;
mod pal;
mod room;
mod sprite;
mod unhsq;

use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    slice,
};

use clap::{Parser, Subcommand};
use frame::Frame;
use itertools::Itertools;
use pal::Pal;
use room::RoomSheet;
use sprite::{Sprite, SpriteSheet};

use crate::{
    bytes_ext::{ReadBytesExt, WriteBytesExt},
    dat_file::DatFile,
    error::Error,
    unhsq::unhsq,
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
    DumpPrt {
        file_name: String,
    },
    /// List the contents of DUNE.DAT
    List,
    /// Decompress RLE-compressed save file
    DecompressSav {
        file_name: String,
    },
    /// Recompress save file
    CompressSav {
        file_name: String,
    },
    DisplaySav {
        file_name: String,
    },
    /// Extracts all resource from DUNE.DAT, decompressing if needed
    ExtractAll,
    /// Extracts a resource from DUNE.DAT without decompressing
    ExtractRaw {
        entry_name: String,
    },
    /// Extracts a resource from DUNE.DAT, decompressing if needed
    Extract {
        entry_name: String,
    },
    /// Extracts sprite resources from a sprite sheet
    ExtractSprites {
        entry_name: String,
    },
    /// Extracts the palette from a sprite sheet
    ExtractPalette {
        entry_name: String,
    },
    /// Extracts font resource
    ExtractFont {
        entry_name: String,
    },
    ExtractCursors,
    ShowPhrases {
        entry_name: String,
    },
    DumpHnm {
        entry_name: String,
    },
    DrawRoom {
        room_sheet_filename: String,
        room_index: usize,
        sprite_sheet_filename: String,
    },
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
    let rle_word = r.read_le_u16()?;
    // The length includes the rle-word and itself but not the first word
    let length = r.read_le_u16()? as usize;

    let rle_byte = rle_word as u8;

    if length != data_len - 2 {
        println!(
            "`{}` is not a Dune save file - invalid length in header.",
            file_name
        );
        return Ok(());
    }

    w.write_le_u16(unk0)?;
    w.write_le_u16(rle_word)?;
    w.write_le_u16(length as u16)?;

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

fn rle_compress_for_save_files<R: Read, W: Write>(
    r: &mut R,
    w: &mut W,
    rle_byte: u8,
) -> std::io::Result<()> {
    #[derive(Default)]
    struct State {
        v: u8,
        reps: usize,
    }

    impl State {
        fn new(v: u8) -> State {
            State { v, reps: 1 }
        }
    }

    let mut state = State::default();

    let mut output = |state: &mut State| -> std::io::Result<()> {
        if state.reps > 2 || state.v == 0xf7 {
            while state.reps > 0 {
                w.write_u8(rle_byte)?;
                w.write_u8(state.reps.min(255) as u8)?;
                w.write_u8(state.v)?;
                state.reps -= state.reps.min(255);
            }
        } else {
            while state.reps > 0 {
                w.write_u8(state.v)?;
                state.reps -= 1;
            }
        }

        Ok(())
    };

    while let Ok(b) = r.read_u8() {
        if state.reps == 0 {
            state = State::new(b);
        } else if state.v == b {
            state.reps += 1;
        } else {
            output(&mut state)?;
            state = State::new(b);
        }

        if state.v == rle_byte {
            output(&mut state)?;
        }
    }

    output(&mut state)
}

fn compress_sav(file_name: &str) -> Result<(), Error> {
    let mut file = File::open(file_name)?;

    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    let mut r = Cursor::new(data);
    let unk0 = r.read_le_u16()?;
    let rle_word = r.read_le_u16()?;

    let rle_byte = rle_word as u8;

    if rle_byte != 0xf7 {
        println!("`{}` is not a valid decompressed save game.", file_name);
        return Ok(());
    }

    let mut w = Vec::<u8>::new();
    rle_compress_for_save_files(&mut r, &mut w, rle_byte)?;

    let out_file_name: String = file_name
        .strip_suffix(".BIN")
        .unwrap_or(file_name)
        .to_owned()
        + ".SAV";

    let mut out_file = BufWriter::new(File::create(&out_file_name)?);
    out_file.write_le_u16(unk0)?;
    out_file.write_le_u16(rle_word)?;
    out_file.write_le_u16((w.len() + 4) as u16)?;
    out_file.write_all(&w)?;

    println!("Compressed `{}` to `{}`", file_name, out_file_name);

    Ok(())
}

#[derive(Debug)]
struct PlaceStatus(u8);

fn display_sav(file_name: &str) -> Result<(), Error> {
    let mut file = File::open(file_name)?;

    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    let data_len = data.len();

    let mut r = Cursor::new(data);
    let mut w = Vec::<u8>::new();

    let _unk0 = r.read_le_u16()?;
    let rle_word = r.read_le_u16()?;
    // The length includes the rle-word and itself but not the first word
    let length = r.read_le_u16()? as usize;

    let rle_byte = rle_word as u8;

    if length != data_len - 2 {
        println!(
            "`{}` is not a Dune save file - invalid length in header.",
            file_name
        );
        return Ok(());
    }

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

    let mut p = Cursor::new(w);

    #[allow(dead_code)]
    #[derive(Debug)]
    struct Sietch {
        first_name: u8,
        last_name: u8,
        desert: u8,
        map_x: u8,
        map_y: u8,
        map_u: u8,
        another_x: u8,
        another_y: u8,
        apparence: u8,
        troop_id: u8,
        status: PlaceStatus,
        discoverable_at_phase: u8,
        unk1: u8,
        unk2: u8,
        unk3: u8,
        unk4: u8,
        spice_field_id: u8,
        unk5: u8,
        spice_density: u8,
        unk6: u8,
        nbr_moiss: u8,
        nbr_orni: u8,
        nbr_knife: u8,
        nbr_guns: u8,
        nbr_mods: u8,
        nbr_atoms: u8,
        nbr_bulbs: u8,
        water: u8,
    }

    let mut sietches = Vec::with_capacity(70);

    for i in 0..70 {
        let offset = 0x4519 + 28 * i;
        p.set_position(offset);
        sietches.push(Sietch {
            first_name: p.read_u8()?,
            last_name: p.read_u8()?,
            desert: p.read_u8()?,
            map_x: p.read_u8()?,
            map_y: p.read_u8()?,
            map_u: p.read_u8()?,
            another_x: p.read_u8()?,
            another_y: p.read_u8()?,
            apparence: p.read_u8()?,
            troop_id: p.read_u8()?,
            status: PlaceStatus(p.read_u8()?),
            discoverable_at_phase: p.read_u8()?,
            unk1: p.read_u8()?,
            unk2: p.read_u8()?,
            unk3: p.read_u8()?,
            unk4: p.read_u8()?,
            spice_field_id: p.read_u8()?,
            unk5: p.read_u8()?,
            spice_density: p.read_u8()?,
            unk6: p.read_u8()?,
            nbr_moiss: p.read_u8()?,
            nbr_orni: p.read_u8()?,
            nbr_knife: p.read_u8()?,
            nbr_guns: p.read_u8()?,
            nbr_mods: p.read_u8()?,
            nbr_atoms: p.read_u8()?,
            nbr_bulbs: p.read_u8()?,
            water: p.read_u8()?,
        });
    }

    let first_names = [
        "Arrakeen", "Carthag", "Tuono", "Habbanya", "Oxtyn", "Tsympo", "Bledan", "Ergsun", "Haga",
        "Cielago", "Sihaya", "Celimyn",
    ];
    let last_names = [
        "(Atreides)",
        "(Harkonnen)",
        "Tabr",
        "Timin",
        "Tuek",
        "Harg",
        "Clam",
        "Tsymyn",
        "Siet",
        "Pyons",
        "Pyort",
    ];

    for (i, s) in sietches.iter().enumerate() {
        let name = format!(
            "{}{}{}",
            first_names
                .get((s.first_name - 1) as usize)
                .cloned()
                .unwrap_or_default(),
            if s.last_name < 3 { ' ' } else { '-' },
            last_names
                .get((s.last_name - 1) as usize)
                .cloned()
                .unwrap_or_default()
        );

        println!("{:2}:\t{name}", i);
    }

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

    let mut pal = Pal::new();
    for i in 0..256 {
        let j = (i * 63 / 256) as u8;
        pal.set(i, (j, j, j));
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

            for i in offset..offset + count {
                let c = (r.read_u8()?, r.read_u8()?, r.read_u8()?);
                pal.set(i as usize, c);
            }
        }
    }

    let file_stem = Path::new(entry_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "SPRITE".into());

    for (i, &offset) in sub_resource_offsets.iter().enumerate() {
        let pos = (toc_position as u64) + (offset as u64);
        r.set_position(pos);

        let src = &r.get_ref()[(r.position() as usize)..];
        let sprite = Sprite::new_from_slice(0, src);

        let width = sprite.width();
        let height = sprite.height();

        if !(1..=320).contains(&width) || !(1..=200).contains(&height) {
            println!("Invalid sprite at resource {i}, offset {offset:04x}");
            continue;
        }

        let mut frame = Frame::new(width, height);
        sprite.draw(&mut frame, 0, 0, false, false, 0, 0).unwrap();

        let filename = format!("{}-{:02}.png", file_stem, i);
        frame.write_png(&filename, &pal).unwrap();
    }
    Ok(())
}

fn extract_palette(dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    println!("Extracting palette from `{}`", entry_name);

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

    for (i, &ofs) in sub_resource_offsets.iter().enumerate() {
        println!("{i:3}: {ofs:04x}");
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

    let mut pal = Pal::new();
    // for i in 0..256 {
    //     let j = (i * 63 / 256) as u8;
    //     pal.set(i, (j, j, j));
    // }

    if toc_position <= 2 {
        println!("No palette in sprite sheet");
        return Ok(());
    }

    for i in 53..56 {
        let offset = sub_resource_offsets[i];
        let pos = (toc_position as u64) + (offset as u64);

        println!("Resource {i} pos {pos:04x}");

        r.set_position(pos);

        // let src = &r.get_ref()[(r.position() as usize)..];

        let zeroes = r.read_le_u16()?;
        let chunk_len = r.read_le_u16()?;
        let offset = r.read_u8()?;
        let count = r.read_u8()?;

        let mut pal = Pal::new();

        for j in 0..count {
            let c = (
                ((r.read_u8()? as u32) * 256 / 63) as u8,
                ((r.read_u8()? as u32) * 256 / 63) as u8,
                ((r.read_u8()? as u32) * 256 / 63) as u8,
            );
            let idx = j as usize + offset as usize;
            println!("{idx:}: {:02x} {:02x} {:02x}", c.0, c.1, c.2);
            pal.set(idx, c);
        }

        const SCALE: usize = 16;
        let mut frame = [0u8; 3 * 16 * SCALE * 16 * SCALE];

        for y in 0..16 * SCALE {
            for x in 0..16 * SCALE {
                let y0 = y / SCALE;
                let x0 = x / SCALE;
                let i = 16 * y0 + x0;
                let c = pal.get(i);
                frame[3 * (y * 16 * SCALE + x) + 0] = c.0;
                frame[3 * (y * 16 * SCALE + x) + 1] = c.1;
                frame[3 * (y * 16 * SCALE + x) + 2] = c.2;
            }
        }

        let file_stem = Path::new(entry_name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "PALETTE".into());

        let path = format!("{file_stem}-palette-{i}.png");

        let f = File::create(&path)?;
        let w = BufWriter::new(f);

        let mut encoder = png::Encoder::new(w, 256, 256);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&frame)?;

        // writeln!(w, "P6 {} {} 255", 16 * SCALE, 16 * SCALE)?;
        // for p in frame {
        //     w.write_u8(p.0)?;
        //     w.write_u8(p.1)?;
        //     w.write_u8(p.2)?;
        // }
    }

    // loop {
    //     let
    // }
    // loop {
    //     let mut v: u16;
    //     loop {
    //         v = r.read_le_u16().unwrap();
    //         if v != 256 {
    //             break;
    //         }
    //         r.set_position(r.position() + 3);
    //     }
    //     if v == 0xffff {
    //         break;
    //     }

    //     let mut count = (v >> 8) & 0xff;
    //     let offset = v & 0xff;

    //     if count == 0 {
    //         count = 256;
    //     }

    //     for i in offset..offset + count {
    //         let c = (r.read_u8()?, r.read_u8()?, r.read_u8()?);
    //         pal.set(i as usize, c);
    //     }
    // }

    // const SCALE: usize = 16;
    // let mut frame = [(0u8, 0u8, 0u8); 16 * SCALE * 16 * SCALE];

    // for y in 0..16 * SCALE {
    //     for x in 0..16 * SCALE {
    //         let y0 = y / SCALE;
    //         let x0 = x / SCALE;
    //         let i = 16 * y0 + x0;
    //         let c = pal.get(i);
    //         frame[y * 16 * SCALE + x] = c;
    //     }
    // }

    // let file_stem = Path::new(entry_name)
    //     .file_stem()
    //     .map(|s| s.to_string_lossy().to_string())
    //     .unwrap_or_else(|| "PALETTE".into());

    // let path = format!("{file_stem}-palette-{SUB_RESOURCE_ID}.ppm");

    // let f = File::create(&path)?;
    // let mut w = BufWriter::new(f);

    // writeln!(w, "P6 {} {} 255", 16 * SCALE, 16 * SCALE)?;
    // for p in frame {
    //     w.write_u8(p.0)?;
    //     w.write_u8(p.1)?;
    //     w.write_u8(p.2)?;
    // }

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

fn extract_cursors() -> Result<(), Error> {
    let f = File::open("DNCDPRG37.EXE")?;
    let mut r = BufReader::new(f);

    for n in 0..6 {
        let offset = 0x11c34 + n * 68;
        r.seek(SeekFrom::Start(offset))?;

        // println!(
        //     "offset: {:#x} seg000:{:04X}",
        //     offset,
        //     offset - 0x11c34 + 0x2584
        // );

        const WIDTH: usize = 16;
        const HEIGHT: usize = 16;
        let mut image_data = [0u8; WIDTH * HEIGHT * 4];

        let _hotspot_x = r.read_le_u16()?;
        let _hotspot_y = r.read_le_u16()?;

        println!("cursor_t ds_{:04x}_cursor = {{", offset - 0x11c34 + 0x2584);
        println!("\t{}, {}, ", _hotspot_x, _hotspot_y);
        println!("\t{{");

        for y in 0..HEIGHT {
            let v = r.read_le_u16()?;
            println!("\t\t0b{:016b},", v);

            for x in 0..WIDTH {
                let b = ((0x8000 >> x) & v) != 0;
                if !b {
                    image_data[4 * (WIDTH * y + x) + 0] = 0;
                    image_data[4 * (WIDTH * y + x) + 1] = 0;
                    image_data[4 * (WIDTH * y + x) + 2] = 0;
                    image_data[4 * (WIDTH * y + x) + 3] = 255;
                }
            }
        }
        println!("\t}}, {{");
        for y in 0..HEIGHT {
            let v = r.read_le_u16()?;
            println!("\t\t0b{:016b},", v);
            for x in 0..WIDTH {
                let b = ((0x8000 >> x) & v) != 0;
                if b {
                    image_data[4 * (WIDTH * y + x) + 0] = 255;
                    image_data[4 * (WIDTH * y + x) + 1] = 255;
                    image_data[4 * (WIDTH * y + x) + 2] = 255;
                    image_data[4 * (WIDTH * y + x) + 3] = 255;
                }
            }
        }
        println!("\t}}");
        println!("}};\n");

        let filename = format!("cursor-{n}.png");
        let path = Path::new(&filename);
        let file = File::create(path)?;
        let w = &mut BufWriter::new(file);

        let mut encoder = png::Encoder::new(w, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image_data)?;

        r.seek_relative(68)?;
    }

    Ok(())
}

#[rustfmt::skip]
const CHARSET_MAP: [char; 0x80] = [
    '\0', '↑', '↓', '→', '←', '_', '\0', '\0', '\0', '¡', '\0', 'ñ', 'Ñ', '\n', 'á', 'ó',
     'ú', 'ò', 'ì', '_', '°', 'ß',  'Ä',  'Ë',  'Ï', 'Ö',  'Ü', 'ä', 'ë',  'ï', 'ö', 'ü',
     ' ', '!', '"', '#', '$', '%',  '&', '\'',  '(', ')',  '*', '+', ',',  '-', '.', '/',
     '0', '1', '2', '3', '4', '5',  '6',  '7',  '8', '9',  ':', ';', '<',  '=', '>', '?',
     '¿', 'A', 'B', 'C', 'D', 'E',  'F',  'G',  'H', 'I',  'J', 'K', 'L',  'M', 'N', 'O',
     'P', 'Q', 'R', 'S', 'T', 'U',  'V',  'W',  'X', 'Y',  'Z', 'â', 'ê',  'î', 'ô', 'û',
     'í', 'a', 'b', 'c', 'd', 'e',  'f',  'g',  'h', 'i',  'j', 'k', 'l',  'm', 'n', 'o',
     'p', 'q', 'r', 's', 't', 'u',  'v',  'w',  'x', 'y',  'z', 'à', 'é',  'è', 'ù', 'ç',
];

#[derive(Debug)]
struct FrameHeader {
    w: u16,
    h: u8,
    flags: u8,
    mode: u8,
}

impl FrameHeader {
    fn new(b: [u8; 4]) -> Self {
        /*
         * | w7 w6 w5 w4 w3 w2 w1 w0 | f6 f5 f4 f3 f2 f1 f0 w8 | h7 h6 h5 h4 h3 h2 h1 h0 | m7 m6 m5 m4 m3 m2 m1 m0 |
         */

        Self {
            w: ((0x1 & (b[1] as u16)) << 8) | (b[0] as u16),
            h: b[2],
            flags: b[1] & 0xfe,
            mode: b[3],
        }
    }

    fn is_compressed(&self) -> bool {
        self.flags & 2 != 0
    }

    fn is_full_frame(&self) -> bool {
        self.flags & 4 != 0
    }
}

fn dump_prt(prt_path: &str) -> Result<(), Error> {
    std::fs::create_dir_all("prt-frames")?;

    let b = std::fs::read(prt_path)?;
    let mut r = Cursor::new(b);
    const FRAMES: usize = 29;
    let mut frame_sizes = [0u16; FRAMES];

    for f in &mut frame_sizes {
        *f = r.read_le_u16()?;
    }

    let mut pal = Pal::new_from_slice(&[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x2A, 0x00, 0x2A, 0x00, 0x00, 0x2A, 0x2A, 0x2A, 0x00, 0x00,
        0x3F, 0x34, 0x14, 0x2A, 0x15, 0x00, 0x2A, 0x2A, 0x2A, 0x15, 0x15, 0x15, 0x15, 0x15, 0x3F,
        0x15, 0x3F, 0x15, 0x15, 0x3F, 0x3F, 0x3F, 0x15, 0x15, 0x3F, 0x15, 0x3F, 0x3F, 0x3F, 0x15,
        0x3F, 0x3F, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x3E, 0x39, 0x1B, 0x3F, 0x3F, 0x26, 0x3F, 0x3D, 0x15, 0x3F, 0x36, 0x1B, 0x3F, 0x2D, 0x00,
        0x36, 0x24, 0x0A, 0x36, 0x18, 0x0A, 0x2D, 0x10, 0x00, 0x24, 0x12, 0x00, 0x1B, 0x12, 0x0F,
        0x1B, 0x0E, 0x08, 0x12, 0x00, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x30, 0x3F, 0x34, 0x28, 0x36, 0x2E, 0x21, 0x2D, 0x28, 0x1A, 0x24, 0x21, 0x13, 0x1B, 0x1A,
        0x0D, 0x13, 0x13, 0x08, 0x04, 0x08, 0x10, 0x04, 0x18, 0x08, 0x00, 0x10, 0x04, 0x00, 0x0C,
        0x0A, 0x00, 0x02, 0x14, 0x1C, 0x1A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0x18, 0x1C,
        0x01, 0x01, 0x02, 0x04, 0x04, 0x05, 0x07, 0x06, 0x09, 0x09, 0x09, 0x0C, 0x0C, 0x0B, 0x0F,
        0x0F, 0x0E, 0x12, 0x11, 0x11, 0x16, 0x14, 0x13, 0x19, 0x17, 0x16, 0x1C, 0x1E, 0x1A, 0x21,
        0x27, 0x1D, 0x27, 0x2D, 0x20, 0x29, 0x33, 0x23, 0x28, 0x39, 0x28, 0x26, 0x3F, 0x33, 0x29,
        0x3F, 0x3F, 0x29, 0x00, 0x00, 0x3F, 0x02, 0x00, 0x3B, 0x05, 0x00, 0x37, 0x08, 0x00, 0x34,
        0x0A, 0x00, 0x30, 0x0C, 0x00, 0x2C, 0x0E, 0x00, 0x29, 0x0E, 0x00, 0x25, 0x0F, 0x00, 0x21,
        0x0F, 0x00, 0x1E, 0x0F, 0x00, 0x1A, 0x0E, 0x00, 0x16, 0x0D, 0x00, 0x13, 0x0B, 0x00, 0x0F,
        0x09, 0x00, 0x0B, 0x07, 0x00, 0x08, 0x3B, 0x30, 0x28, 0x3B, 0x31, 0x28, 0x3B, 0x31, 0x29,
        0x3C, 0x32, 0x2A, 0x3C, 0x33, 0x2B, 0x3C, 0x33, 0x2C, 0x3C, 0x34, 0x2C, 0x3D, 0x34, 0x2E,
        0x3D, 0x35, 0x2E, 0x3D, 0x36, 0x2F, 0x08, 0x10, 0x0A, 0x3F, 0x3F, 0x3F, 0x2E, 0x3D, 0x32,
        0x22, 0x30, 0x28, 0x1F, 0x2A, 0x26, 0x26, 0x34, 0x2C, 0x08, 0x0A, 0x10, 0x0B, 0x0B, 0x10,
        0x0C, 0x0B, 0x10, 0x0D, 0x0B, 0x10, 0x0F, 0x0B, 0x10, 0x10, 0x0B, 0x10, 0x10, 0x0B, 0x0F,
        0x10, 0x0B, 0x0D, 0x10, 0x0B, 0x0C, 0x10, 0x0B, 0x0B, 0x32, 0x32, 0x32, 0x2F, 0x2A, 0x37,
        0x1E, 0x16, 0x3E, 0x1C, 0x0B, 0x3F, 0x0F, 0x10, 0x0B, 0x3D, 0x34, 0x2E, 0x0C, 0x10, 0x0B,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x3F, 0x3F, 0x3F,
    ]);

    for i in 0..FRAMES {
        let frame_offset =
            frame_sizes.iter().take(i).map(|v| *v as u64).sum::<u64>() + (2 * FRAMES as u64);

        r.set_position(frame_offset);

        let mut hsq_header_buf = [0u8; 6];
        r.read_exact(&mut hsq_header_buf)?;

        let checksum = hsq_header_buf
            .bytes()
            .flatten()
            .fold(0u8, |acc, x| acc.wrapping_add(x));
        assert!(checksum == 0xab);

        r.seek_relative(-6)?;

        let unpacked_len = r.read_le_u16()?;
        let _zero = r.read_u8()?;
        let _packed_len = r.read_le_u16()?;
        let _checksum = r.read_u8()?;

        let mut unpacked_buffer = Box::new([0u8; 65536]);
        let remaining_slice = r.split().1;
        unhsq(remaining_slice, &mut *unpacked_buffer);

        let mut r = Cursor::new(&unpacked_buffer[0..unpacked_len as usize]);

        let _pal_len = r.read_le_u16()?;

        apply_palette_update(&mut r, &mut pal)?;
        while r.read_u8()? == 0xff {}
        r.seek_relative(-1)?;

        let data = r.split().1;
        let sprite = Sprite::new_from_slice(i, data);

        let mut frame = Frame::new(sprite.width(), sprite.height());
        sprite.draw(&mut frame, 0, 0, false, false, 0, 0)?;

        let filename = format!("PRT-{i:02}.png");
        frame.write_png(&filename, &pal)?;
        println!(
            "Write {} x {} image to {}",
            sprite.width(),
            sprite.height(),
            filename
        );
    }

    Ok(())
}

fn dump_hnm(dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    let data = dat_file.read(entry_name)?;
    let file_stem = Path::new(entry_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "MOVIE".into());

    let mut r = Cursor::new(data.as_slice());
    let header_size = r.read_le_u16()?;

    let mut pal = Pal::new_from_slice(&[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x2A, 0x00, 0x2A, 0x00, 0x00, 0x2A, 0x2A, 0x2A, 0x00, 0x00,
        0x3F, 0x34, 0x14, 0x2A, 0x15, 0x00, 0x2A, 0x2A, 0x2A, 0x15, 0x15, 0x15, 0x15, 0x15, 0x3F,
        0x15, 0x3F, 0x15, 0x15, 0x3F, 0x3F, 0x3F, 0x15, 0x15, 0x3F, 0x15, 0x3F, 0x3F, 0x3F, 0x15,
        0x3F, 0x3F, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x3E, 0x39, 0x1B, 0x3F, 0x3F, 0x26, 0x3F, 0x3D, 0x15, 0x3F, 0x36, 0x1B, 0x3F, 0x2D, 0x00,
        0x36, 0x24, 0x0A, 0x36, 0x18, 0x0A, 0x2D, 0x10, 0x00, 0x24, 0x12, 0x00, 0x1B, 0x12, 0x0F,
        0x1B, 0x0E, 0x08, 0x12, 0x00, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x30, 0x3F, 0x34, 0x28, 0x36, 0x2E, 0x21, 0x2D, 0x28, 0x1A, 0x24, 0x21, 0x13, 0x1B, 0x1A,
        0x0D, 0x13, 0x13, 0x08, 0x04, 0x08, 0x10, 0x04, 0x18, 0x08, 0x00, 0x10, 0x04, 0x00, 0x0C,
        0x0A, 0x00, 0x02, 0x14, 0x1C, 0x1A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0x18, 0x1C,
        0x01, 0x01, 0x02, 0x04, 0x04, 0x05, 0x07, 0x06, 0x09, 0x09, 0x09, 0x0C, 0x0C, 0x0B, 0x0F,
        0x0F, 0x0E, 0x12, 0x11, 0x11, 0x16, 0x14, 0x13, 0x19, 0x17, 0x16, 0x1C, 0x1E, 0x1A, 0x21,
        0x27, 0x1D, 0x27, 0x2D, 0x20, 0x29, 0x33, 0x23, 0x28, 0x39, 0x28, 0x26, 0x3F, 0x33, 0x29,
        0x3F, 0x3F, 0x29, 0x00, 0x00, 0x3F, 0x02, 0x00, 0x3B, 0x05, 0x00, 0x37, 0x08, 0x00, 0x34,
        0x0A, 0x00, 0x30, 0x0C, 0x00, 0x2C, 0x0E, 0x00, 0x29, 0x0E, 0x00, 0x25, 0x0F, 0x00, 0x21,
        0x0F, 0x00, 0x1E, 0x0F, 0x00, 0x1A, 0x0E, 0x00, 0x16, 0x0D, 0x00, 0x13, 0x0B, 0x00, 0x0F,
        0x09, 0x00, 0x0B, 0x07, 0x00, 0x08, 0x3B, 0x30, 0x28, 0x3B, 0x31, 0x28, 0x3B, 0x31, 0x29,
        0x3C, 0x32, 0x2A, 0x3C, 0x33, 0x2B, 0x3C, 0x33, 0x2C, 0x3C, 0x34, 0x2C, 0x3D, 0x34, 0x2E,
        0x3D, 0x35, 0x2E, 0x3D, 0x36, 0x2F, 0x08, 0x10, 0x0A, 0x3F, 0x3F, 0x3F, 0x2E, 0x3D, 0x32,
        0x22, 0x30, 0x28, 0x1F, 0x2A, 0x26, 0x26, 0x34, 0x2C, 0x08, 0x0A, 0x10, 0x0B, 0x0B, 0x10,
        0x0C, 0x0B, 0x10, 0x0D, 0x0B, 0x10, 0x0F, 0x0B, 0x10, 0x10, 0x0B, 0x10, 0x10, 0x0B, 0x0F,
        0x10, 0x0B, 0x0D, 0x10, 0x0B, 0x0C, 0x10, 0x0B, 0x0B, 0x32, 0x32, 0x32, 0x2F, 0x2A, 0x37,
        0x1E, 0x16, 0x3E, 0x1C, 0x0B, 0x3F, 0x0F, 0x10, 0x0B, 0x3D, 0x34, 0x2E, 0x0C, 0x10, 0x0B,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x3F, 0x3F, 0x3F,
    ]);

    apply_palette_update(&mut r, &mut pal)?;

    while r.read_u8()? == 0xff {}
    r.seek_relative(-1)?;

    let toc_start = r.position() as u16;
    let entry_count = (header_size - toc_start) / 4;
    let mut entries = Vec::with_capacity(entry_count as usize);

    for _ in 0..entry_count {
        entries.push(r.read_le_u32()?);
    }
    let frame_count = entry_count - 1;

    if false {
        for i in 0..frame_count {
            let entry_size = entries[i as usize + 1] - entries[i as usize];
            let offset = header_size as u64 + entries[i as usize] as u64;
            r.seek(SeekFrom::Start(offset))?;

            print!("{i:4}:");
            let len = u32::min(entry_size, 32);
            for _ in 0..len {
                print!(" {:02x}", r.read_u8()?);
            }
            if entry_size > 32 {
                print!(" ..");
            }
            println!();
        }
    }

    let mut frame = Frame::new(320, 200);

    for i in 0..frame_count {
        let offset = header_size as u64 + entries[i as usize] as u64;
        r.seek(SeekFrom::Start(offset))?;

        let _frame_size = r.read_le_u16()?;
        let mut frame_header = [0u8; 4];

        loop {
            r.read_exact(&mut frame_header)?;

            match &frame_header[0..2] {
                [b'p', b'l'] => {
                    let buf: &[u8; 2] = frame_header[2..4].try_into().unwrap();
                    let size = u16::from_le_bytes(*buf);
                    let position = r.position();
                    assert!(size >= 4);
                    apply_palette_update(&mut r, &mut pal)?;
                    r.set_position(position + (size as u64) - 4);
                }
                [b's', b'd'] => {
                    let buf: &[u8; 2] = frame_header[2..4].try_into().unwrap();
                    let size = u16::from_le_bytes(*buf);
                    assert!(size >= 4);
                    r.seek_relative(size as i64 - 4)?;
                }
                _ => {
                    let header = FrameHeader::new(frame_header);

                    if true {
                        print!("{:4}: {:?} ", i, header);
                        let mut sep = false;

                        if header.w == 0 || header.h == 0 {
                            print!("zero size frame");
                            sep = true;
                        }

                        if sep {
                            print!(", ");
                        }

                        if header.is_compressed() {
                            print!("compressed frame")
                        } else {
                            print!("uncompressed frame");
                        }

                        print!(", ");
                        if header.is_full_frame() {
                            print!("full frame");
                        } else {
                            print!("partial frame");
                        }
                        println!();
                    }

                    if header.w > 0 && header.h > 0 {
                        let mut unpacked_buffer = Box::new([0u8; 65536]);
                        let r2 = if header.is_compressed() {
                            let mut hsq_header_buf = [0u8; 6];

                            r.read_exact(&mut hsq_header_buf)?;

                            r.seek_relative(-6)?;

                            let _unpacked_len = r.read_le_u16()?;
                            let _zero = r.read_u8()?;
                            let _packed_len = r.read_le_u16()?;
                            let _checksum = r.read_u8()?;

                            dbg!(_unpacked_len);
                            dbg!(_zero);
                            dbg!(_packed_len);
                            dbg!(_checksum);

                            let remaining_slice = r.split().1;
                            unhsq(remaining_slice, &mut *unpacked_buffer);
                            // print!("\t\t");
                            // for i in 0..32 {
                            //     print!("{:02x} ", unpacked_buffer[i]);
                            // }
                            // println!();

                            Cursor::new(unpacked_buffer.as_slice())
                        } else {
                            r.clone()
                        };

                        let mut r = r2;

                        let mut x = 0;
                        let mut y = 0;
                        if !header.is_full_frame() {
                            x = r.read_le_u16()?;
                            y = r.read_le_u16()?;

                            println!("frame offset: {:?}", (x, y));
                        }

                        let sprite = Sprite::new_from_slice(i as usize, r.split().1);

                        dbg!(
                            sprite.width(),
                            sprite.height(),
                            sprite.pal_offset(),
                            sprite.rle()
                        );

                        sprite.draw(&mut frame, x as usize, y as usize, false, false, 0, 0)?;

                        // let src = &r.get_ref()[(r.position() as usize)..];
                        // let dst_x = x as usize;
                        // let dst_y = y as usize;
                        // let w = header.w as usize;
                        // let h = header.h as usize;

                        // if i == 0 {
                        //     // assert!(header.is_full_frame());
                        //     assert!(header.w > 0);
                        //     assert!(header.h > 0);

                        //     // dbg!((dst_x, dst_y, w, h));

                        //     image_width = dst_x + w;
                        //     image_height = dst_y + h;

                        //     // dbg!(image_width, image_height);

                        //     image_data = vec![0u8; image_width * image_height * 4];
                        // }

                        // todo!();
                        // sprite::draw(
                        //     &mut image_data,
                        //     image_width,
                        //     image_height,
                        //     src,
                        //     dst_x,
                        //     dst_y,
                        //     w,
                        //     h,
                        //     w,
                        //     header.flags,
                        //     header.mode,
                        //     &pal,
                        // )?;
                    }

                    if true {
                        std::fs::create_dir_all(format!("hnm-frames/{}", file_stem))?;

                        let filename =
                            format!("hnm-frames/{}/{}-{:04}.png", file_stem, file_stem, i);

                        frame.write_png(&filename, &pal).unwrap();
                    }

                    break;
                }
            }
        }
    }

    Ok(())
}

fn apply_palette_update(r: &mut Cursor<&[u8]>, pal: &mut Pal) -> Result<(), Error> {
    loop {
        let offset = r.read_u8()? as usize;
        let mut count = r.read_u8()? as usize;

        if offset == 1 && count == 0 {
            r.seek_relative(3)?;
            continue;
        }
        if offset == 0xff && count == 0xff {
            break;
        }
        if count == 0 {
            count = 256;
        }

        for i in 0..count {
            let cr = r.read_u8()?;
            let cg = r.read_u8()?;
            let cb = r.read_u8()?;

            pal.set(offset + i, (cr, cg, cb));
        }
    }

    Ok(())
}

fn show_phrases(dat_file: &mut DatFile, entry_name: &str) -> Result<(), Error> {
    let data = dat_file.read(entry_name)?;
    let mut r = Cursor::new(data.as_slice());

    let offset = r.read_le_u16()?;
    let count = (offset / 2) as usize;
    assert!(count > 0);

    let mut offsets = Vec::with_capacity(count);
    offsets.push(offset);
    for _ in 1..count {
        offsets.push(r.read_le_u16()?);
    }

    let mut s = String::new();
    for (i, offset) in offsets.iter().cloned().enumerate() {
        r.set_position(offset as u64);

        loop {
            let b = r.read_u8()?;
            if b == 0xff {
                break;
            }

            #[allow(clippy::if_same_then_else)]
            if b >= 0xf0 {
            } else if b >= 0xd0 {
            } else if b >= 0xa0 {
            } else if b >= 0x90 {
                if b == 0x91 {
                    s += format!("{{byte_{:X}h}}", r.read_u8()?).as_str();
                } else if b == 0x92 {
                    s += format!("{{word_{:X}h}}", r.read_u8()?).as_str();
                }
            } else if b >= 0x80 {
                let v = if b == 0x80 {
                    r.read_le_u16()?
                } else {
                    (b - 0x80) as u16
                };
                s += format!("{{str_{:X}h}}", v).as_str();
            } else if b == 0x06 {
                s.push_str("{whisper}");
            } else if b == 0x0d {
                s.push_str("\\n");
            } else {
                s.push(CHARSET_MAP[b as usize]);
            }
        }

        println!("{:3}: \"{}\"", i, s);

        s.clear();
    }

    Ok(())
}

fn decompress_4bpp_rle(src: &mut Cursor<&[u8]>, w: u16, h: u16, dst: &mut Cursor<&mut [u8]>) {
    for _ in 0..h {
        let mut line_remain = 2 * w.div_ceil(4) as i16;
        loop {
            let cmd = src.read_u8().unwrap() as i16;
            let count: i16;

            if cmd & 0x80 != 0 {
                count = 257 - cmd;
                let value = src.read_u8().unwrap();
                for _ in 0..count {
                    dst.write_u8(value).unwrap();
                }
            } else {
                count = cmd + 1;
                for _ in 0..count {
                    let v = src.read_u8().unwrap();
                    dst.write_u8(v).unwrap();
                }
            }
            line_remain -= count;

            if line_remain <= 0 {
                break;
            }
        }
    }
}

fn draw_room(
    dat_file: &mut DatFile,
    room_sheet_filename: &str,
    room_index: usize,
    sprite_sheet_filename: &str,
) -> Result<(), Error> {
    let room_sheet_data = dat_file
        .read(&format!("{}.SAL", room_sheet_filename))
        .expect("Entry not found");

    let room_sheet = RoomSheet::new(&room_sheet_data).unwrap();

    let Some(room) = room_sheet.at(room_index) else {
        println!("Invalid room index {}", room_index);
        return Ok(());
    };

    let sprite_data = dat_file
        .read(&format!("{}.HSQ", sprite_sheet_filename))
        .expect("Entry not found");

    let sprite_sheet = SpriteSheet::new(&sprite_data).unwrap();

    let mut pal = Pal::new();
    sprite_sheet.apply_palette_update(&mut pal).unwrap();

    let mut frame = Frame::new(320, 200);
    room.draw(&mut frame, &sprite_sheet);

    let filename = format!(
        "{}-{:02}-{}.png",
        room_sheet_filename, room_index, sprite_sheet_filename
    );

    frame.write_png(&filename, &pal)?;

    Ok(())
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    let out_path = cli.out_path;
    let mut dat_file = DatFile::open(&cli.dat_path).expect("Failed to open DUNE.DAT");

    match &cli.command {
        Commands::DumpPrt { file_name } => dump_prt(file_name)?,
        Commands::List => list(&mut dat_file),
        Commands::DecompressSav { file_name } => {
            decompress_sav(file_name)?;
        }
        Commands::CompressSav { file_name } => {
            compress_sav(file_name)?;
        }
        Commands::DisplaySav { file_name } => {
            display_sav(file_name)?;
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
        Commands::ExtractPalette { entry_name } => {
            extract_palette(&mut dat_file, entry_name)?;
        }
        Commands::ExtractFont { entry_name } => {
            extract_font(&mut dat_file, entry_name)?;
        }
        Commands::ExtractCursors => {
            extract_cursors()?;
        }
        Commands::ShowPhrases { entry_name } => {
            show_phrases(&mut dat_file, entry_name)?;
        }
        Commands::DumpHnm { entry_name } => {
            dump_hnm(&mut dat_file, entry_name)?;
        }
        Commands::DrawRoom {
            room_sheet_filename,
            room_index,
            sprite_sheet_filename,
        } => {
            draw_room(
                &mut dat_file,
                room_sheet_filename,
                *room_index,
                sprite_sheet_filename,
            )?;
        }
    }
    Ok(())
}
