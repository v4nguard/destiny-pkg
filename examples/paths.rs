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

    for (p, path) in &package_manager.package_paths {
        println!(
            "{p:04x}: platform={platform:?}, name={name:?}, language={language:?}, id={id}, patch={patch}, path={path:?}, filename={filename:?}",
            platform=path.platform, name=path.name, language=path.language, id=path.id, patch=path.patch, path=path.path, filename=path.filename,
        );
    }

    Ok(())
}
