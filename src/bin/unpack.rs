use destiny2_pkg::package::Package;
use std::fs::File;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let package = Package::open(&std::env::args().nth(1).unwrap())?;
    std::fs::create_dir("./files/").ok();

    for (i, e) in package.entries().enumerate() {
        if e.reference != u32::MAX {
            print!(
                "{i} 0x{:x} - p={:x} f={} ",
                e.file_size,
                (e.reference >> 13) & 0x3ff,
                e.reference & 0x1fff
            );
        } else {
            print!("{i} 0x{:x} - ", e.file_size);
        }
        let ext = match (e.file_type, e.file_subtype) {
            (26, 6) => {
                println!("WWise WAVE Audio");
                "wav".to_string()
            }
            (26, 7) => {
                println!("Havok File");
                "hkf".to_string()
            }
            (27, _) => {
                println!("CriWare USM Video");
                "usm".to_string()
            }
            (33, _) => {
                println!("DirectX Bytecode Header");
                "cso.header".to_string()
            }
            (32, _) => {
                println!("Texture Header");
                "texture.header".to_string()
            }
            (40, _) | (48, 1) | (48, 2) => {
                println!("Texture Data");
                "texture.data".to_string()
            }
            (41, _) => {
                let ty = match e.file_subtype {
                    0 => "fragment".to_string(),
                    1 => "vertex".to_string(),
                    6 => "compute".to_string(),
                    u => format!("unk{u}"),
                };
                println!("DirectX Bytecode Data ({})", ty);

                format!("cso.{ty}")
            }
            (8, _) => {
                println!("8080 structure file");
                "8080".to_string()
            }
            _ => {
                println!("Unknown {}/{}", e.file_type, e.file_subtype);
                "bin".to_string()
            }
        };

        let data = match package.read_entry(i) {
            Ok(data) => data,
            Err(e) => {
                eprintln!(
                    "Failed to extract entry {}/{}: {e}",
                    i,
                    package.entries().count() - 1
                );
                continue;
            }
        };

        let mut o = File::create(format!("files/{i}.{ext}"))?;
        o.write_all(&data)?;
    }

    Ok(())
}
