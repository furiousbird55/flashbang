use flashbang::devices::{Disk, DiskChild, FlashDecision, discover_disks};
use flashbang::format::format_size;
use flashbang::images::{ImageFile, inspect_image};
use flashbang::writer::{WritePlan, build_write_plan};
use std::env;
use std::path::PathBuf;

enum ReportSection {
    Ready,
    NeedsAttention,
    Hidden,
}

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

    print_section("Ready to flash:", disks, image, ReportSection::Ready);
    println!();

    print_section(
        "Needs attention:",
        disks,
        image,
        ReportSection::NeedsAttention,
    );
    println!();

    print_section("Hidden by default:", disks, image, ReportSection::Hidden);
}

fn print_section(title: &str, disks: &[Disk], image: Option<&ImageFile>, section: ReportSection) {
    println!("{title}");

    let image_size = image.map(|image| image.size_bytes);
    let mut found_any = false;

    for disk in disks {
        let decision = disk.flash_decision(image_size);

        let belongs_here = match section {
            ReportSection::Ready => decision == FlashDecision::ReadyToFlash,
            ReportSection::NeedsAttention => {
                decision != FlashDecision::ReadyToFlash
                    && decision != FlashDecision::HiddenByDefault
            }
            ReportSection::Hidden => decision == FlashDecision::HiddenByDefault,
        };

        if belongs_here {
            print_disk(disk, image);
            found_any = true;
        }
    }

    if !found_any {
        println!("  none");
    }
}

fn print_disk(disk: &Disk, image: Option<&ImageFile>) {
    let image_size = image.map(|image| image.size_bytes);
    let decision = disk.flash_decision(image_size);

    println!();
    println!("- {}", disk.path);
    println!("  name: {}", disk.name);
    println!("  model: {}", disk.model_label());
    println!("  size: {}", format_size(disk.size_bytes));
    println!("  transport: {}", disk.transport_label());
    println!("  removable: {}", yes_no(disk.removable));
    println!("  read-only: {}", yes_no(disk.read_only));
    println!("  safety decision: {}", decision.label());
    println!("  suggested action: {}", decision.suggested_action());

    if let Some(image) = image {
        if decision != FlashDecision::HiddenByDefault {
            println!(
                "  image fits: {}",
                yes_no(image.fits_in_bytes(disk.size_bytes))
            );

            let plan = build_write_plan(image, disk);
            print_write_plan(&plan);
        }
    }

    let mounted_filesystems = disk.mounted_filesystems();

    if mounted_filesystems.is_empty() {
        println!("  contains mounted filesystems: no");
    } else {
        println!("  contains mounted filesystems: yes");

        for filesystem in &mounted_filesystems {
            println!(
                "    - {} at {}",
                filesystem.device_path, filesystem.mountpoint
            );
        }
    }

    if !disk.children.is_empty() {
        println!("  child devices:");

        for child in &disk.children {
            print_child(child, 4);
        }
    }
}

fn print_write_plan(plan: &WritePlan) {
    println!("  write plan:");
    println!("    image: {}", plan.image_name);
    println!("    image path: {}", plan.image_path.display());
    println!("    image size: {}", format_size(plan.image_size_bytes));
    println!("    target: {}", plan.target_path);
    println!("    target model: {}", plan.target_model);
    println!("    target size: {}", format_size(plan.target_size_bytes));
    println!("    readiness: {}", plan.decision.label());

    if plan.preparation_steps.is_empty() {
        println!("    preparation: none");
    } else {
        println!("    preparation:");

        for step in &plan.preparation_steps {
            println!("      - {}", step.label());
        }
    }

    println!("    final actions:");

    for action in &plan.final_actions {
        println!("      - {}", action.label());
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
