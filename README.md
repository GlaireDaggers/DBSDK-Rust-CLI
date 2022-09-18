# DBSDK-Rust-CLI
Command-line utility for building DreamBox games written in Rust

## Installation

```
cargo install dbsdk-cli
```

## Usage

```
dbsdk-cli new <NAME>
  Creates a new DBSDK Rust project with the given name. This sets up appropriate Cargo.toml, src/lib.rs, and .cargo/config.toml
  
dbsdk-cli build [OPTIONS] <INPATH>
  Build the DBSDK Rust project at the given path
  OPTIONS:
  -l, --label <LABEL>     Set the disc volume label
  -o, --outpath <PATH>    Set the build output path
  -p, --profile <PROFILE> Set the build profile ("debug" or "release")
```
