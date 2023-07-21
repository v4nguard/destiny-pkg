use clap::Parser;
use clap_num::maybe_hex;
use destiny_pkg::package::classify_file;
use destiny_pkg::{PackageManager, PackageVersion, TagHash};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, disable_version_flag(true))]
struct Args {
    /// Path to packages directory
    packages_path: String,

    #[arg(value_parser = maybe_hex::<u32>)]
    reference: u32,

    /// Don't extract any files, just print them
    #[arg(short, long, default_value = "false")]
    dry_run: bool,

    /// Directory to extract to (default: ./out/pkg_name)
    #[arg(short)]
    output_dir: Option<String>,

    /// Version of the package to extract
    #[arg(short, value_enum)]
    version: PackageVersion,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut package_manager = PackageManager::new(args.packages_path, args.version, true)?;

    for (p, i, e) in package_manager.get_all_by_reference(args.reference) {
        let pkg_path = package_manager.package_paths.get(&p).unwrap();
        let pkg_name = PathBuf::from(pkg_path)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let out_dir = args
            .output_dir
            .clone()
            .unwrap_or_else(|| format!("./out/{pkg_name}"));

        let ext = if args.version == PackageVersion::Destiny2PreBeyondLight {
            classify_file(e.file_type, e.file_subtype)
        } else {
            "bin".to_string()
        };

        std::fs::create_dir_all(&out_dir).ok();
        let ref_hash = TagHash(e.reference);
        if ref_hash.is_pkg_file() {
            println!(
                "{:04x}/{i} 0x{:04x} - Reference {ref_hash:?} / r=0x{:x} (type={}, subtype={}, ext={ext})",
                p, e.file_size, ref_hash.0, e.file_type, e.file_subtype
            );
        } else {
            println!(
                "{:04x}/{i} 0x{:04x} - r=0x{:x} (type={}, subtype={}, ext={ext})",
                p, e.file_size, ref_hash.0, e.file_type, e.file_subtype
            );
        }

        if !args.dry_run {
            let data = match package_manager.read_entry(p, i) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to extract entry {:04x}/{}: {e}", p, i,);
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
