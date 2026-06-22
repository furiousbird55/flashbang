use flashbang::devices::{discover_disks, Disk, DiskStatus};

fn main() {
    println!("Flashbang device discovery");
    println!();

    let disks = match discover_disks() {
        Ok(disks) => disks,
        Err(error) => {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
    };

    print_disk_report(&disks);
}

fn print_disk_report(disks: &[Disk]) {
    if disks.is_empty() {
        println!("No disks found.");
        return;
    }

    println!("Flash candidates:");

    let candidates: Vec<&Disk> = disks
        .iter()
        .filter(|disk| disk.status() == DiskStatus::FlashCandidate)
        .collect();

    if candidates.is_empty() {
        println!("  none");
    } else {
        for disk in candidates {
            print_disk(disk);
        }
    }

    println!();
    println!("Hidden by default:");

    let hidden: Vec<&Disk> = disks
        .iter()
        .filter(|disk| disk.status() == DiskStatus::HiddenByDefault)
        .collect();

    if hidden.is_empty() {
        println!("  none");
    } else {
        for disk in hidden {
            print_disk(disk);
        }
    }
}

fn print_disk(disk: &Disk) {
    println!();
    println!("- {}", disk.path);
    println!("  name: {}", disk.name);
    println!("  model: {}", disk.model_label());
    println!("  size: {:.1} GiB", disk.size_gib());
    println!("  transport: {}", disk.transport_label());
    println!("  removable: {}", yes_no(disk.removable));
    println!("  read-only: {}", yes_no(disk.read_only));
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}