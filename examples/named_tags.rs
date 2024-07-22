use clap::Parser;
use destiny_pkg::{package::PackagePlatform, GameVersion, PackageManager};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, disable_version_flag(true))]
struct Args {
    /// Path to packages directory
    packages_path: String,

    /// Version of the package
    #[arg(short, value_enum)]
    version: GameVersion,

    #[arg(short, value_enum)]
    platform: Option<PackagePlatform>,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    let package_manager = PackageManager::new(args.packages_path, args.version, args.platform)?;

    for tag in &package_manager.named_tags {
        let activity_pkg = &package_manager.package_paths[&tag.hash.pkg_id()];
        let activity_pkg = &activity_pkg.filename;

        println!(
            "{activity_pkg}: {} - {} (D2Class_{:08x})",
            tag.name,
            tag.hash,
            tag.class_hash.to_be(),
        );
    }

    Ok(())
}
