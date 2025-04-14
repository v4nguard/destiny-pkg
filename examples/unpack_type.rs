use std::{fs::File, io::Write};

use clap::Parser;
use tiger_pkg::{
    package::{classify_file_prebl, PackagePlatform},
    DestinyVersion, GameVersion, PackageManager, TagHash,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, disable_version_flag(true))]
struct Args {
    /// Path to packages directory
    packages_path: String,

    #[arg(long = "type")]
    entry_type: u8,
    #[arg(long = "subtype")]
    entry_subtype: Option<u8>,

    /// Directory to extract to
    #[arg(short, default_value = "./out/")]
    output_dir: String,

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

    for (tag, _) in package_manager.get_all_by_type(args.entry_type, args.entry_subtype) {
        let Some(entry) = package_manager.get_entry(tag) else {
            eprintln!("Tag {} does not exist!", tag);
            continue;
        };
        let pkg_path = package_manager.package_paths.get(&tag.pkg_id()).unwrap();
        let pkg_name = &pkg_path.filename;

        let out_dir = args.output_dir.clone();

        let ext = if args.version == GameVersion::Destiny(DestinyVersion::Destiny2Shadowkeep) {
            classify_file_prebl(entry.file_type, entry.file_subtype)
        } else {
            "bin".to_string()
        };

        std::fs::create_dir_all(&out_dir).ok();
        let ref_hash = TagHash(entry.reference);
        if ref_hash.is_pkg_file() {
            println!(
                "{pkg_name} {:04x}/{} 0x{:04x} - Reference {ref_hash:?} / r=0x{:x} (type={}, subtype={}, ext={ext})",
                tag.pkg_id(), tag.entry_index(), entry.file_size, ref_hash.0, entry.file_type, entry.file_subtype
            );
        } else {
            println!(
                "{pkg_name} {:04x}/{} 0x{:04x} - r=0x{:x} (type={}, subtype={}, ext={ext})",
                tag.pkg_id(),
                tag.entry_index(),
                entry.file_size,
                ref_hash.0,
                entry.file_type,
                entry.file_subtype
            );
        }

        let data = match package_manager.read_tag(tag) {
            Ok(data) => data,
            Err(e) => {
                eprintln!(
                    "Failed to extract entry {:04x}/{}: {e}",
                    tag.pkg_id(),
                    tag.entry_index()
                );

                continue;
            }
        };

        let mut o = File::create(format!(
            "{out_dir}/{tag}_ref-{:08X}_{}_{}.{ext}",
            entry.reference, entry.file_type, entry.file_subtype
        ))?;
        o.write_all(&data)?;
    }

    Ok(())
}
