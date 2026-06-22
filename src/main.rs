// FLASHBANG_MAIN_WITH_DEVICE_WRITE_V1

use flashbang::devices::{Disk, DiskChild, FlashDecision, discover_disks};
use flashbang::format::format_size;
use flashbang::images::{ImageFile, inspect_image};
use flashbang::writer::{
    WritePlan, build_write_plan, run_preparation_steps, write_image_to_device, write_image_to_file,
};
use std::env;
use std::path::PathBuf;

enum ReportSection {
    Ready,
    NeedsAttention,
    Hidden,
}

struct ProgramArgs {
    image: Option<ImageFile>,
    copy_to: Option<PathBuf>,
    write_device: Option<String>,
    assume_yes: bool,
}

fn main() {
    println!("Flashbang device discovery");
    println!();

    let args = match read_program_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("Argument error: {error}");
            std::process::exit(1);
        }
    };

    if let Some(image) = &args.image {
        print_image_report(image);
        println!();

        if let Some(output_path) = &args.copy_to {
            run_test_copy(image, output_path);
            println!();
        }
    } else {
        print_usage();
    }

    let disks = match discover_disks() {
        Ok(disks) => disks,
        Err(error) => {
            eprintln!("Disk discovery error: {error}");
            std::process::exit(1);
        }
    };

    print_disk_report(&disks, args.image.as_ref());

    if let (Some(image), Some(target_path)) = (&args.image, &args.write_device) {
        println!();
        run_device_write(image, &disks, target_path, args.assume_yes);
    }
}

fn print_usage() {
    println!("No image selected.");
    println!("Tip:");
    println!("  cargo run -- /path/to/image.iso");
    println!();
    println!("Test copy mode:");
    println!("  cargo run -- /path/to/image.iso --copy-to /tmp/flashbang-test.img");
    println!();
    println!("Device write mode:");
    println!("  sudo ./target/debug/flashbang /path/to/image.iso --write-device /dev/sdX --yes");
    println!();
}

fn read_program_args() -> Result<ProgramArgs, String> {
    let mut raw_args = env::args_os().skip(1);

    let mut image_path: Option<PathBuf> = None;
    let mut copy_to: Option<PathBuf> = None;
    let mut write_device: Option<String> = None;
    let mut assume_yes = false;

    while let Some(arg) = raw_args.next() {
        if arg == "--copy-to" {
            let Some(path) = raw_args.next() else {
                return Err("--copy-to needs an output path".to_string());
            };

            copy_to = Some(PathBuf::from(path));
        } else if arg == "--write-device" {
            let Some(path) = raw_args.next() else {
                return Err("--write-device needs a target device path".to_string());
            };

            write_device = Some(path.to_string_lossy().to_string());
        } else if arg == "--yes" || arg == "-y" {
            assume_yes = true;
        } else if image_path.is_none() {
            image_path = Some(PathBuf::from(arg));
        } else {
            return Err(format!(
                "unexpected argument: {}",
                PathBuf::from(arg).display()
            ));
        }
    }

    if copy_to.is_some() && write_device.is_some() {
        return Err("use either --copy-to or --write-device, not both".to_string());
    }

    let image = match image_path {
        Some(path) => Some(inspect_image(path)?),
        None => None,
    };

    if copy_to.is_some() && image.is_none() {
        return Err("--copy-to requires an image path".to_string());
    }

    if write_device.is_some() && image.is_none() {
        return Err("--write-device requires an image path".to_string());
    }

    Ok(ProgramArgs {
        image,
        copy_to,
        write_device,
        assume_yes,
    })
}

fn run_test_copy(image: &ImageFile, output_path: &PathBuf) {
    println!("Test copy:");
    println!(
        "- writing {} to {}",
        image.file_name_label(),
        output_path.display()
    );

    let mut last_printed_percent = 0_u64;

    let result = write_image_to_file(image, output_path, |progress| {
        print_progress(
            progress.percent(),
            progress.bytes_written,
            progress.total_bytes,
            &mut last_printed_percent,
        );
    });

    match result {
        Ok(()) => println!("  result: test copy complete"),
        Err(error) => {
            eprintln!("  result: test copy failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_device_write(image: &ImageFile, disks: &[Disk], target_path: &str, assume_yes: bool) {
    println!("Device write requested:");
    println!("- image: {}", image.file_name_label());
    println!("- target: {target_path}");

    let Some(disk) = disks.iter().find(|disk| disk.path == target_path) else {
        eprintln!("Error: target device was not found by Flashbang: {target_path}");
        std::process::exit(1);
    };

    let plan = build_write_plan(image, disk);

    println!("- target model: {}", plan.target_model);
    println!("- target size: {}", format_size(plan.target_size_bytes));
    println!("- readiness: {}", plan.decision.label());
    println!("- execution: {}", plan.execution_mode.label());

    if !plan.execution_mode.can_eventually_write() {
        eprintln!("Error: this target is not writable in its current state.");
        std::process::exit(1);
    }

    if !assume_yes {
        eprintln!("Error: refusing to write without --yes.");
        eprintln!("Re-run with:");
        eprintln!(
            "  sudo ./target/debug/flashbang {} --write-device {target_path} --yes",
            image.path.display()
        );
        std::process::exit(1);
    }

    if !plan.preparation_steps.is_empty() {
        println!("Preparing target:");

        for step in &plan.preparation_steps {
            println!("- {}", step.label());
        }

        if let Err(error) = run_preparation_steps(&plan.preparation_steps) {
            eprintln!("Error during preparation: {error}");
            std::process::exit(1);
        }

        println!("Preparation complete.");
    }

    let refreshed_disks = match discover_disks() {
        Ok(disks) => disks,
        Err(error) => {
            eprintln!("Disk rediscovery error after preparation: {error}");
            std::process::exit(1);
        }
    };

    let Some(refreshed_disk) = refreshed_disks.iter().find(|disk| disk.path == target_path) else {
        eprintln!("Error: target disappeared after preparation: {target_path}");
        std::process::exit(1);
    };

    let refreshed_decision = refreshed_disk.flash_decision(Some(image.size_bytes));

    if refreshed_decision != FlashDecision::ReadyToFlash {
        eprintln!(
            "Error: target is still not ready after preparation: {}",
            refreshed_decision.label()
        );
        std::process::exit(1);
    }

    println!("Writing image to device:");
    println!("- this will overwrite {target_path}");

    let mut last_printed_percent = 0_u64;

    let result = write_image_to_device(image, target_path, |progress| {
        print_progress(
            progress.percent(),
            progress.bytes_written,
            progress.total_bytes,
            &mut last_printed_percent,
        );
    });

    match result {
        Ok(()) => println!("Device write complete."),
        Err(error) => {
            eprintln!("Device write failed: {error}");
            std::process::exit(1);
        }
    }
}

fn print_progress(
    percent: f64,
    bytes_written: u64,
    total_bytes: u64,
    last_printed_percent: &mut u64,
) {
    let percent = percent.floor() as u64;

    if percent >= *last_printed_percent + 10 || percent == 100 {
        println!(
            "  progress: {:>3}% ({}/{})",
            percent,
            format_size(bytes_written),
            format_size(total_bytes)
        );

        *last_printed_percent = percent;
    }
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
    println!("    execution: {}", plan.execution_mode.label());

    if plan.preparation_steps.is_empty() {
        println!("    preparation: none");
    } else {
        println!("    preparation:");

        for step in &plan.preparation_steps {
            println!("      - {}", step.label());
        }
    }

    if plan.final_actions.is_empty() {
        println!("    final actions: none");
    } else {
        println!("    final actions:");

        for action in &plan.final_actions {
            println!("      - {}", action.label());
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
