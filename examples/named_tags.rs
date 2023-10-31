use std::path::Path;

use clap::Parser;
use destiny_pkg::{PackageManager, PackageVersion};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, disable_version_flag(true))]
struct Args {
    /// Path to packages directory
    packages_path: String,

    /// Version of the package
    #[arg(short, value_enum)]
    version: PackageVersion,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    let package_manager = PackageManager::new(args.packages_path, args.version)?;

    for tag in &package_manager.named_tags {
        let activity_pkg = &package_manager.package_paths[&tag.hash.pkg_id()];
        let activity_pkg = Path::new(activity_pkg)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();

        println!(
            "{activity_pkg}: {} - {} (D2Class_{:08x})",
            tag.name,
            tag.hash,
            tag.class_hash.to_be(),
        );
    }

    Ok(())
}
