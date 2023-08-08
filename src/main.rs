mod bytes_ext;
mod dat_file;
mod unhsq;

use std::{
    fs::File,
    io::{Cursor, Write},
    path::PathBuf,
    process::exit,
};

use clap::{Parser, Subcommand};

use crate::{bytes_ext::ReadBytesExt, dat_file::DatFile, unhsq::unhsq};

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

fn extract_all(dat_file: &mut DatFile) {
    let entry_names = dat_file
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect::<Vec<_>>();
    for name in entry_names.iter() {
        extract(dat_file, name);
    }
}

fn extract_raw(dat_file: &mut DatFile, entry_name: &str) {
    let data = dat_file.read(entry_name).expect("Entry not found");

    let mut f = File::create(entry_name).expect("Unable to open output file");

    f.write_all(data.as_slice())
        .expect("Failed to write output file");

    println!("Extracted to `{}`", entry_name);
}

fn is_compressed(header: &[u8]) -> bool {
    if header.len() < 6 {
        return false;
    }

    let checksum: u8 = header.iter().take(6).fold(0, |acc, &x| acc.wrapping_add(x));

    checksum == 0xab && header[2] == 0
}

fn extract(dat_file: &mut DatFile, entry_name: &str) {
    println!("Extracting `{}`", entry_name);

    let data = dat_file.read(entry_name).expect("Entry not found");

    let is_compressed = is_compressed(&data);

    println!("Compressed: {}", if is_compressed { "Yes" } else { "No" });

    if is_compressed {
        let mut reader = Cursor::new(&data);
        let unpacked_length = reader.read_le_u16().unwrap();
        _ = reader.read_u8();
        let packed_length = reader.read_le_u16().unwrap();
        _ = reader.read_u8();

        println!("Packed length:   {}", packed_length);
        println!("Unpacked length: {}", unpacked_length);

        if packed_length as usize != data.len() {
            println!("Packed length does not match resource size");
            exit(1);
        }

        let mut unpacked_data = vec![0; unpacked_length as usize];

        unhsq(&data[6..], &mut unpacked_data);

        let mut path: PathBuf = entry_name.into();
        path.set_extension("BIN");

        let mut f = File::create(&path).expect("Unable to open output file");

        f.write_all(unpacked_data.as_slice())
            .expect("Failed to write output file");

        println!("Extracted to `{}`", path.display());
    } else {
        let mut f = File::create(entry_name).expect("Unable to open output file");

        f.write_all(data.as_slice())
            .expect("Failed to write output file");

        println!("Extracted to `{}`", entry_name);
    }
}

fn main() {
    let cli = Cli::parse();

    let mut dat_file = DatFile::open(&cli.dat_path).expect("Failed to open DUNE.DAT");

    match &cli.command {
        Commands::List => list(&mut dat_file),
        Commands::ExtractAll => {
            extract_all(&mut dat_file);
        }
        Commands::ExtractRaw { entry_name } => {
            extract_raw(&mut dat_file, entry_name);
        }
        Commands::Extract { entry_name } => {
            extract(&mut dat_file, entry_name);
        }
    }
}
