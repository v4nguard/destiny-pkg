use std::{fs::File, io::Write, path::PathBuf};

use clap::Parser;
use destiny_pkg::{package::classify_file_prebl, PackageVersion, TagHash};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, disable_version_flag(true))]
struct Args {
    /// Package to extract
    package: String,

    /// Don't extract any files, just print them
    #[arg(short, long, default_value = "false")]
    dry_run: bool,

    /// Directory to extract to (default: ./out/pkg_name)
    #[arg(short)]
    output_dir: Option<String>,

    /// Version of the package to extract
    #[arg(short, value_enum)]
    version: PackageVersion,

    /// Only extract 8080 files
    #[arg(long)]
    only_8080: bool,

    /// Don't print files
    #[arg(short, long)]
    silent: bool,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    if args.silent && args.dry_run {
        eprintln!("Warning: silent and dry_run are both enabled, nothing will be printed");
    }

    let pkg_name = PathBuf::from(args.package.clone())
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let package = args.version.open(&args.package)?;

    let out_dir = args
        .output_dir
        .unwrap_or_else(|| format!("./out/{pkg_name}"));

    std::fs::create_dir_all(&out_dir).ok();

    println!("PKG {:04x}_{}", package.pkg_id(), package.patch_id());
    for (i, e) in package.entries().iter().enumerate() {
        if (e.file_type != 8 && e.file_type != 16) && args.only_8080 {
            continue;
        }

        if !args.silent {
            print!("{}/{} - ", e.file_type, e.file_subtype);
        }
        let ref_hash = TagHash(e.reference);

        let ext = if args.version == PackageVersion::Destiny2Shadowkeep {
            classify_file_prebl(e.file_type, e.file_subtype)
        } else {
            "bin".to_string()
        };

        if !args.silent {
            if ref_hash.is_pkg_file() {
                println!(
                "{i} 0x{:04x} - Reference {ref_hash:?} / r=0x{:x} (type={}, subtype={}, ext={ext})",
                e.file_size, ref_hash.0, e.file_type, e.file_subtype
            );
            } else {
                println!(
                    "{i} 0x{:04x} - r=0x{:x} (type={}, subtype={}, ext={ext})",
                    e.file_size, ref_hash.0, e.file_type, e.file_subtype
                );
            }
        }

        if !args.dry_run {
            let data: Vec<u8> = match package.read_entry(i) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!(
                        "Failed to extract entry {}/{}: {e}",
                        i,
                        package.entries().len() - 1
                    );
                    continue;
                }
            };

            let mut o = File::create(format!(
                "{out_dir}/{i}_{:08x}_t{}_s{}.{ext}",
                e.reference, e.file_type, e.file_subtype
            ))?;
            o.write_all(&data)?;
        }
    }

    Ok(())
}
