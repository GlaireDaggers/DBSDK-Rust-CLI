use std::fmt::format;
use std::fs::{File, self, OpenOptions};
use std::io::{prelude::*, SeekFrom};
use std::path::Path;
use std::process::Command;

use cargo_toml::Manifest;
use clap::{Parser,Subcommand};
use fs_extra::dir::CopyOptions;

#[derive(Parser)]
struct Args {
    #[clap(subcommand)]
    action: Action
}

#[derive(Subcommand)]
enum Action {
    New { name: String },
    Build { inpath: String, #[clap(short, long)] outpath: Option<String>, #[clap(short, long)] profile: Option<String>, #[clap(short, long)] label: Option<String> },
}

fn new(name: &String) {
    if Path::new("./Cargo.toml").exists() {
        println!("Cargo.toml already exists in the current path!");
        std::process::exit(1);
    }

    // create a new Cargo.toml here
    // tried cargo_toml_builder but crate_type support is broken so plain old templated string it is

    let cargo_toml = format(format_args!(r#"[package]
name = "{}"
version = "1.0.0"
authors = [""]

[lib]
crate-type = ["cdylib"]

[dependencies]
"#, name));

    let mut cargo_toml_file = File::create(Path::new("Cargo.toml")).unwrap();
    cargo_toml_file.write(cargo_toml.as_bytes()).unwrap();
    println!("Cargo.toml written");

    // create .cargo/config.toml

    fs::create_dir_all(".cargo").unwrap();
    let mut main_rs_file = File::create(Path::new(".cargo/config.toml")).unwrap();
    main_rs_file.write(r#"[build]
target = "wasm32-unknown-unknown"
rustflags = [
    "-C", "link-arg=--max-memory=16777216",
    "-C", "link-arg=--export-table",
]"#.as_bytes()).unwrap();
    println!(".cargo/config.toml written");

    // create src/lib.rs

    fs::create_dir_all("src").unwrap();
    let mut main_rs_file = File::create(Path::new("src/lib.rs")).unwrap();
    main_rs_file.write(r#"#[no_mangle]
pub fn main(_: i32, _: i32) -> i32 {
    return 0;
}"#.as_bytes()).unwrap();
    println!("src/lib.rs written");
}

fn ensure_mkisofs() {
    // ensure mkiso-fs is present and up to date
    Command::new("cargo").args(["install", "mkisofs-rs"]).status().unwrap();
}

fn build_profile(inpath: &String, outpath: &String, libname: &String, disclabel: &String, is_release: bool) {
    let buildstatus = 
        if is_release { Command::new("cargo").args(["build", "--target", "wasm32-unknown-unknown", "--release"]).status() }
        else { Command::new("cargo").args(["build", "--target", "wasm32-unknown-unknown"]).status() };
    
    if buildstatus.is_err() {
        let err = buildstatus.unwrap_err();
        println!("Build command failed: {}", err);
        std::process::exit(1);
    }

    // output will be at target/wasm32-unknown-unknown/{debug | release}/[libname].wasm
    // copy it to [outpath]/{debug | release}/main.wasm

    let wasm_path =
        if is_release { format!("{}/target/wasm32-unknown-unknown/release/{}.wasm", inpath, libname) }
        else { format!("{}/target/wasm32-unknown-unknown/debug/{}.wasm", inpath, libname) };

    let binpathroot_str =
        if is_release { format!("{}/release", outpath) }
        else { format!("{}/debug", outpath) };

    let binpathroot = Path::new(binpathroot_str.as_str());
    let binpath = format!("{}/main.wasm", binpathroot_str);

    println!("Creating dir: {:?}", binpathroot);
    println!("Copying {} to {}", wasm_path, binpath);

    fs::create_dir_all(binpathroot).unwrap();
    fs::copy(Path::new(wasm_path.as_str()), Path::new(binpath.as_str())).unwrap();

    // if there's a content/ folder, copy it as well
    let content_path = format!("{}/content", inpath);

    if Path::new(content_path.as_str()).exists() {
        let mut copy_options = CopyOptions::new();
        copy_options.overwrite = true;
        let mut in_paths = Vec::new();
        in_paths.push(content_path);
        fs_extra::copy_items(&in_paths, binpathroot, &copy_options).unwrap();
    }

    // build final {debug | release}.iso
    let isopath_str = 
        if is_release { format!("{}/release.iso", outpath) }
        else { format!("{}/debug.iso", outpath) };

    let isopath = Path::new(isopath_str.as_str());
    let mkisostatus = Command::new("mkisofs-rs").args(["--no-boot", "-o", isopath_str.as_str(), binpathroot_str.as_str()]).status();
    if mkisostatus.is_err() {
        let err = mkisostatus.unwrap_err();
        println!("mkisofs-rs failed: {}", err);
        std::process::exit(1);
    }

    // hack: mkisofs-rs doesn't allow passing custom volume identifiers, so we just rewrite the primary volume identifier after the fact
    // first volume descriptor is the Primary Volume Descriptor, starting at 0x8000, and the volume identifier is at 0x8028 (32 bytes long, space-padded)
    let mut isofile = OpenOptions::new().write(true).open(isopath).unwrap();
    isofile.seek(SeekFrom::Start(0x8028)).unwrap();
    isofile.write_all(disclabel.as_bytes()).unwrap();

    // space-pad to 32 bytes
    for _ in 0..(32 - disclabel.len()) {
        isofile.write_all(&[0x20]).unwrap();
    }

    print!("ISO created at {}", isopath_str);
}

fn build(inpath: &String, outpath: &Option<String>, profile: &Option<String>, label: &Option<String>) {
    ensure_mkisofs();

    let build = "build".to_string();
    let dbg = "debug".to_string();

    let outpath_real = outpath.as_ref().unwrap_or(&build);
    let profile_real = profile.as_ref().unwrap_or(&dbg);

    let cargopath = format!("{}/Cargo.toml", inpath);

    // look for Cargo.toml
    if !Path::new(cargopath.as_str()).exists() {
        println!("Could not find Cargo.toml at input path");
        std::process::exit(1);
    }

    // parse Cargo.toml
    let cargo = Manifest::from_path(cargopath).unwrap();

    // default lib name is project name (but convert kebab case to snake case)
    let mut libname = str::replace(&cargo.package.unwrap().name, "-", "_");

    // otherwise, if cargo defines a lib name, use that
    match cargo.lib {
        Some(lib) => {
            match lib.name {
                Some(ln) => {
                    libname = ln;
                },
                None => {
                }
            };
        },
        None => {
        }
    };

    let disclabel = label.as_ref().unwrap_or(&libname);

    // great, now execute a build
    match profile_real.as_str() {
        "debug" => {
            build_profile(inpath, outpath_real, &libname, disclabel, false);
        },
        "release" => {
            build_profile(inpath, outpath_real, &libname, disclabel, true);
        },
        _ => {
            println!("Unrecognized build profile: {}", profile_real);
            std::process::exit(1);
        }
    }
}

fn main() {
    let args = Args::parse();

    match &args.action {
        Action::New { name } => {
            new(name);
        },
        Action::Build { inpath, outpath, profile, label } => {
            build(inpath, outpath, profile, label);
        }
    }
}