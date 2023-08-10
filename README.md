# Dune Extract

Resource extractor for Cryo's Dune (CD version).

## Synopsis

```sh
./dune-extract [--dat-path <DAT_PATH>] list

./dune-extract [--dat-path <DAT_PATH>] extract <RESOURCE_NAME>

./dune-extract [--dat-path <DAT_PATH>] extract-raw <RESOURCE_NAME>

./dune-extract [--dat-path <DAT_PATH>] extract-sprites <RESOURCE_NAME>
```

Resource files will by default be extracted to the directory `dump`.

When using `extract`, compressed resource files with the extension `.HSQ` will be exported with the extension `.BIN`.

Sprites will exported as PNG files. Not all sprite resources have an included palette and will be exported with a gray-scale palette instead.

## Installation

Install [the Rust toolchain](https://www.rust-lang.org/tools/install), `git clone` this project to your development folder, and run `cargo build --release` in the project folder. The binary should be compiled to the folder `./target/release/dune-extract`.

## Usage

Place the `DUNE.DAT` file from Dune and the `dune-extract` binary in the same folder, or let `dune-extract` know where `DUNE.DAT` is using the `--dat-path` parameter.

```
Usage: dune-extract [OPTIONS] <COMMAND>

Commands:
  list             List the contents of DUNE.DAT
  extract-all      Extracts all resource from DUNE.DAT, decompressing if needed
  extract-raw      Extracts a resource from DUNE.DAT without decompressing
  extract          Extracts a resource from DUNE.DAT, decompressing if needed
  extract-sprites  Extracts sprite resources from a sprite sheet
  help             Print this message or the help of the given subcommand(s)

Options:
      --dat-path <DAT_PATH>
      --out-path <OUT_PATH>  [default: dump]
  -h, --help                 Print help
```
