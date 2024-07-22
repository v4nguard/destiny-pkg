use std::collections::HashMap;

use clap::Parser;
use destiny_pkg::{package::PackagePlatform, GameVersion, PackageManager};
use rustc_hash::FxHashMap;

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

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let package_manager = PackageManager::new(args.packages_path, args.version, args.platform)?;
    let mut totals: HashMap<(u8, u8), (usize, usize)> = Default::default();
    let mut references: FxHashMap<u32, (usize, usize)> = Default::default();

    for (_, entries) in package_manager.package_entry_index {
        for entry in entries {
            if entry.file_type == 8 || entry.file_type == 16 {
                let e = references.entry(entry.reference).or_default();
                e.0 += 1;
                e.1 += entry.file_size as usize;
            }

            let e = totals
                .entry((entry.file_type, entry.file_subtype))
                .or_default();
            e.0 += entry.file_size as usize;
            e.1 += 1;
        }
    }

    let mut resorted_totals: Vec<((u8, u8), (usize, usize))> = totals.into_iter().collect();
    resorted_totals.sort_by_key(|((t, s), _)| ((*t as u32) << 16) | *s as u32);
    for ((ftype, fsubtype), (size, count)) in resorted_totals {
        println!(
            "{ftype}.{fsubtype} - {} ({} files, {} per file on average)",
            format_file_size(size),
            split_thousands(count, '\''),
            format_file_size(size / count)
        );
    }

    println!();
    println!("Tag reference types ({} unique):", references.len());
    let mut resorted_references: Vec<(u32, (usize, usize))> = references.into_iter().collect();
    resorted_references.sort_by_key(|(r, _)| *r);
    for (reference, (count, size)) in resorted_references {
        println!(
            " {:08X} {} \t({}, {} per file on average)",
            reference.to_be(),
            split_thousands(count, '\''),
            format_file_size(size),
            format_file_size(size / count)
        );
    }

    Ok(())
}

fn split_thousands(v: usize, separator: char) -> String {
    // 12345 => 54321
    let s: Vec<char> = v.to_string().chars().rev().collect();

    // 54321 => 543 21
    let c: Vec<String> = s.chunks(3).map(|s| s.iter().collect::<String>()).collect();

    // 543 21 => 12 345 => 12'345
    c.into_iter()
        .collect::<Vec<String>>()
        .join(&separator.to_string())
        .chars()
        .rev()
        .collect()
}

fn format_file_size(size: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    const GB: usize = MB * 1024;
    const TB: usize = GB * 1024;

    if size < KB {
        format!("{} B", size)
    } else if size < MB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else if size < GB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size < TB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else {
        format!("{:.2} TB", size as f64 / TB as f64)
    }
}
