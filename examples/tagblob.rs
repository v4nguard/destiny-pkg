use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::{atomic::AtomicUsize, Arc},
};

use clap::Parser;
use itertools::Itertools;
use parking_lot::Mutex;
use pbr::ProgressBar;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tiger_pkg::{
    package::PackagePlatform, DestinyVersion, GameVersion, MarathonVersion, PackageManager,
    TagHash, Version,
};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, disable_version_flag(true))]
struct Args {
    /// Path to packages directory
    packages_path: String,

    /// Version of the package to extract
    #[arg(short, value_enum)]
    version: GameVersion,

    #[arg(short, value_enum)]
    platform: Option<PackagePlatform>,
}

pub fn wwise_bank_type(version: GameVersion) -> (u8, u8) {
    match version {
        GameVersion::Destiny(v) => match v {
            DestinyVersion::DestinyInternalAlpha => (0, 15),
            DestinyVersion::DestinyTheTakenKing
            | DestinyVersion::DestinyFirstLookAlpha
            | DestinyVersion::DestinyRiseOfIron => (0, 20),
            DestinyVersion::Destiny2Beta
            | DestinyVersion::Destiny2Forsaken
            | DestinyVersion::Destiny2Shadowkeep => (26, 5),
            DestinyVersion::Destiny2BeyondLight
            | DestinyVersion::Destiny2WitchQueen
            | DestinyVersion::Destiny2Lightfall
            | DestinyVersion::Destiny2TheFinalShape => (26, 6),
        },
        // Same as post-BeyondLight
        GameVersion::Marathon(MarathonVersion::MarathonAlpha) => (26, 6),
    }
}

fn main() -> anyhow::Result<()> {
    tracy_client::Client::start();
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(tracing_tracy::TracyLayer::default()),
    )
    .expect("setup tracy layer");

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    let package_manager = PackageManager::new(args.packages_path, args.version, args.platform)?;

    let mut entries = vec![];
    entries.extend(package_manager.get_all_by_type(8, None)); // Tag
    entries.extend(package_manager.get_all_by_type(16, None)); // TagGlobal
    let (bank_type, bank_subtype) = wwise_bank_type(package_manager.version);
    entries.extend(package_manager.get_all_by_type(bank_type, Some(bank_subtype))); // WwiseBank

    let blob_size = entries.iter().fold(0usize, |size, (_, e)| {
        size + ((e.file_size + 0xf) & !0xf) as usize
    });

    let blob = Arc::new(vec![0u8; blob_size]);
    println!("Blob size: {}", blob_size);

    let index = Mutex::new(BufWriter::new(File::create("tagblob.txt")?));

    let pos = Arc::new(AtomicUsize::new(0));
    let current_index = Arc::new(AtomicUsize::new(0));
    let mut pb = ProgressBar::new(entries.len() as u64);

    // Group entries by package index
    let entries_bucketed = entries
        .into_iter()
        .map(|(tag, e)| (tag.pkg_id(), (tag.entry_index(), e)))
        .into_group_map();

    let pos_clone = pos.clone();
    let current_index_clone = current_index.clone();
    std::thread::spawn(move || {
        while pos_clone.load(std::sync::atomic::Ordering::Relaxed) < blob_size {
            pb.set(current_index_clone.load(std::sync::atomic::Ordering::Relaxed) as u64);
        }
    });

    entries_bucketed
        .par_iter()
        .for_each(|(package_id, entries)| {
            let package = package_manager
                .version
                .open(&package_manager.package_paths[package_id].path)
                .unwrap();
            let _pt = tracing::info_span!("Processing package").entered();

            for (entry_index, e) in entries {
                let _et = tracing::info_span!("Read entry").entered();
                let offset = {
                    let _ = tracing::info_span!("Writing index entry").entered();
                    // lock the index while incrementing the position so they don't fall out of order
                    let mut idx = index.lock();
                    let o = pos.fetch_add(
                        ((e.file_size + 0xf) & !0xf) as usize,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                    writeln!(
                        idx,
                        "tag={} offset=0x{:X} size=0x{:X}",
                        TagHash::new(*package_id, *entry_index),
                        o,
                        e.file_size
                    )
                    .ok();
                    o
                };

                // Safety: we know the offset is within the bounds of the blob, and that no other threads will write to this specific slice
                let destination = unsafe {
                    let ptr = blob.as_ptr().add(offset);
                    std::slice::from_raw_parts_mut(ptr as *mut u8, e.file_size as usize)
                };
                if let Ok(d) = package.read_entry(*entry_index as usize) {
                    let _ = tracing::info_span!("Copy").entered();
                    destination.copy_from_slice(&d);
                }

                current_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        });

    std::fs::write("tagblob.bin", blob.as_slice())?;

    Ok(())
}
