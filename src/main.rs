use flashbang::devices::{Disk, DiskChild, DiskStatus, discover_disks};

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

    println!("Ready to flash:");

    let ready: Vec<&Disk> = disks
        .iter()
        .filter(|disk| disk.status() == DiskStatus::ReadyToFlash)
        .collect();

    if ready.is_empty() {
        println!("  none");
    } else {
        for disk in ready {
            print_disk(disk);
        }
    }

    println!();
    println!("Needs unmount first:");

    let needs_unmount: Vec<&Disk> = disks
        .iter()
        .filter(|disk| disk.status() == DiskStatus::NeedsUnmount)
        .collect();

    if needs_unmount.is_empty() {
        println!("  none");
    } else {
        for disk in needs_unmount {
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

    if disk.has_mounts() {
        println!("  contains mounted filesystems: yes");

        for mountpoint in disk.all_mountpoints() {
            println!("    - {mountpoint}");
        }
    } else {
        println!("  contains mounted filesystems: no");
    }

    if !disk.children.is_empty() {
        println!("  child devices:");

        for child in &disk.children {
            print_child(child, 4);
        }
    }
}

fn print_child(child: &DiskChild, indent: usize) {
    let padding = " ".repeat(indent);

    println!("{padding}- {}", child.path);
    println!("{padding}  name: {}", child.name);
    println!("{padding}  type: {}", child.device_type);
    println!("{padding}  size: {:.1} GiB", child.size_gib());

    if child.mountpoints.is_empty() {
        println!("{padding}  mounted: no");
    } else {
        println!("{padding}  mounted: yes");

        for mountpoint in &child.mountpoints {
            println!("{padding}    - {mountpoint}");
        }
    }

    for nested_child in &child.children {
        print_child(nested_child, indent + 4);
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
