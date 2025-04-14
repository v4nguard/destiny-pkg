use std::{fs::File, io::Write};

use clap::Parser;
use clap_num::maybe_hex;
use tiger_pkg::{
    package::{classify_file_prebl, PackagePlatform},
    DestinyVersion, GameVersion, PackageManager, TagHash,
};

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
    version: GameVersion,

    #[arg(short, value_enum)]
    platform: Option<PackagePlatform>,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    let package_manager = PackageManager::new(args.packages_path, args.version, args.platform)?;

    for (t, e) in package_manager.get_all_by_reference(args.reference) {
        let pkg_path = package_manager.package_paths.get(&t.pkg_id()).unwrap();
        let pkg_name = &pkg_path.filename;

        let out_dir = args
            .output_dir
            .clone()
            .unwrap_or_else(|| format!("./out/{pkg_name}"));

        let ext = if args.version == GameVersion::Destiny(DestinyVersion::Destiny2Shadowkeep) {
            classify_file_prebl(e.file_type, e.file_subtype)
        } else {
            "bin".to_string()
        };

        std::fs::create_dir_all(&out_dir).ok();
        let ref_hash = TagHash(e.reference);
        if ref_hash.is_pkg_file() {
            println!(
                "{:04x}/{} 0x{:04x} - Reference {ref_hash:?} / r=0x{:x} (type={}, subtype={}, ext={ext})",
                t.pkg_id(), t.entry_index(), e.file_size, ref_hash.0, e.file_type, e.file_subtype
            );
        } else {
            println!(
                "{:04x}/{} 0x{:04x} - r=0x{:x} (type={}, subtype={}, ext={ext})",
                t.pkg_id(),
                t.entry_index(),
                e.file_size,
                ref_hash.0,
                e.file_type,
                e.file_subtype
            );
        }

        if !args.dry_run {
            let data = match package_manager.read_tag(t) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!(
                        "Failed to extract entry {:04x}/{}: {e}",
                        t.pkg_id(),
                        t.entry_index()
                    );
                    continue;
                }
            };

            let mut o = File::create(format!(
                "{out_dir}/{}_{:08x}_t{}_s{}.{ext}",
                t.entry_index(),
                e.reference,
                e.file_type,
                e.file_subtype
            ))?;
            o.write_all(&data)?;
        }
    }

    Ok(())
}
