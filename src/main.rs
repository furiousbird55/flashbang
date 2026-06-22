use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<BlockDevice>,
}

#[derive(Debug, Deserialize)]
struct BlockDevice {
    name: String,
    path: Option<String>,
    rm: bool,
    size: u64,
    ro: bool,
    model: Option<String>,
    tran: Option<String>,

    #[serde(rename = "type")]
    device_type: String,
}

fn main() {
    println!("Flashbang device discovery");
    println!();

    match discover_disks() {
        Ok(disks) => print_disks(&disks),
        Err(error) => {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
    }
}

fn discover_disks() -> Result<Vec<BlockDevice>, String> {
    let output = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--output",
            "NAME,PATH,SIZE,RM,RO,TYPE,MODEL,TRAN",
        ])
        .output()
        .map_err(|error| format!("failed to run lsblk: {error}"))?;

    if !output.status.success() {
        return Err(format!("lsblk failed with status: {}", output.status));
    }

    let parsed: LsblkOutput = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse lsblk JSON: {error}"))?;

    let disks = parsed
        .blockdevices
        .into_iter()
        .filter(|device| device.device_type == "disk")
        .collect();

    Ok(disks)
}

fn print_disks(disks: &[BlockDevice]) {
    if disks.is_empty() {
        println!("No disks found.");
        return;
    }

    println!("Detected disks:");

    for disk in disks {
        let path = disk.path.as_deref().unwrap_or("unknown path");

        let model = disk
            .model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown model");

        let transport = disk
            .tran
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown transport");

        let flashable = is_flash_candidate(disk);

        println!();
        println!("- {path}");
        println!("  name: {}", disk.name);
        println!("  model: {model}");
        println!("  size: {}", format_size(disk.size));
        println!("  transport: {transport}");
        println!("  removable: {}", yes_no(disk.rm));
        println!("  read-only: {}", yes_no(disk.ro));

        if flashable {
            println!("  flashbang status: candidate");
        } else {
            println!("  flashbang status: hidden by default");
        }
    }
}

fn is_flash_candidate(disk: &BlockDevice) -> bool {
    disk.device_type == "disk" && disk.rm && !disk.ro
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn format_size(bytes: u64) -> String {
    let gib = bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    format!("{gib:.1} GiB")
}