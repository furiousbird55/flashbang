use flashbang::devices::{Disk, DiskChild, DiskStatus, discover_disks};
use flashbang::images::{ImageFile, inspect_image};
use std::env;
use std::path::PathBuf;

fn main() {
    println!("Flashbang device discovery");
    println!();

    let image = match read_image_argument() {
        Ok(image) => image,
        Err(error) => {
            eprintln!("Image error: {error}");
            std::process::exit(1);
        }
    };

    if let Some(image) = &image {
        print_image_report(image);
        println!();
    } else {
        println!("No image selected.");
        println!("Tip: run with an image path, for example:");
        println!("  cargo run -- /path/to/image.iso");
        println!();
    }

    let disks = match discover_disks() {
        Ok(disks) => disks,
        Err(error) => {
            eprintln!("Disk discovery error: {error}");
            std::process::exit(1);
        }
    };

    print_disk_report(&disks, image.as_ref());
}

fn read_image_argument() -> Result<Option<ImageFile>, String> {
    let Some(path) = env::args_os().nth(1) else {
        return Ok(None);
    };

    let path = PathBuf::from(path);
    inspect_image(path).map(Some)
}

fn print_image_report(image: &ImageFile) {
    println!("Selected image:");
    println!("- {}", image.file_name_label());
    println!("  path: {}", image.path.display());
    println!("  size: {}", format_size(image.size_bytes));
}

fn print_disk_report(disks: &[Disk], image: Option<&ImageFile>) {
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
            print_disk(disk, image);
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
            print_disk(disk, image);
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
            print_disk(disk, image);
        }
    }
}

fn print_disk(disk: &Disk, image: Option<&ImageFile>) {
    println!();
    println!("- {}", disk.path);
    println!("  name: {}", disk.name);
    println!("  model: {}", disk.model_label());
    println!("  size: {}", format_size(disk.size_bytes));
    println!("  transport: {}", disk.transport_label());
    println!("  removable: {}", yes_no(disk.removable));
    println!("  read-only: {}", yes_no(disk.read_only));

    if let Some(image) = image {
        println!(
            "  image fits: {}",
            yes_no(image.fits_in_bytes(disk.size_bytes))
        );
    }

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
    println!("{padding}  size: {}", format_size(child.size_bytes));

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

fn format_size(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    if bytes as f64 >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}
