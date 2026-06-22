// FLASHBANG_WRITER_WITH_VERIFICATION_V1

use crate::devices::{Disk, FlashDecision};
use crate::images::ImageFile;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const COPY_BUFFER_SIZE: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct WritePlan {
    pub image_path: PathBuf,
    pub image_name: String,
    pub image_size_bytes: u64,
    pub target_path: String,
    pub target_model: String,
    pub target_size_bytes: u64,
    pub decision: FlashDecision,
    pub execution_mode: ExecutionMode,
    pub preparation_steps: Vec<PreparationStep>,
    pub final_actions: Vec<FinalAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Immediate,
    AfterPreparation,
    Blocked,
}

#[derive(Debug, Clone)]
pub enum PreparationStep {
    Unmount {
        device_path: String,
        mountpoint: String,
    },
}

#[derive(Debug, Clone)]
pub enum FinalAction {
    WriteImageBytes,
    SyncTarget,
}

#[derive(Debug, Clone, Copy)]
pub struct WriteProgress {
    pub bytes_written: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct VerifyProgress {
    pub bytes_checked: u64,
    pub total_bytes: u64,
}

impl WriteProgress {
    pub fn percent(&self) -> f64 {
        percent(self.bytes_written, self.total_bytes)
    }
}

impl VerifyProgress {
    pub fn percent(&self) -> f64 {
        percent(self.bytes_checked, self.total_bytes)
    }
}

fn percent(done: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (done as f64 / total as f64) * 100.0
    }
}

impl ExecutionMode {
    pub fn label(&self) -> &'static str {
        match self {
            ExecutionMode::Immediate => "can execute immediately",
            ExecutionMode::AfterPreparation => "can execute after preparation",
            ExecutionMode::Blocked => "blocked",
        }
    }

    pub fn can_eventually_write(&self) -> bool {
        matches!(
            self,
            ExecutionMode::Immediate | ExecutionMode::AfterPreparation
        )
    }
}

impl PreparationStep {
    pub fn label(&self) -> String {
        match self {
            PreparationStep::Unmount {
                device_path,
                mountpoint,
            } => {
                format!("unmount {device_path} from {mountpoint}")
            }
        }
    }
}

impl FinalAction {
    pub fn label(&self) -> &'static str {
        match self {
            FinalAction::WriteImageBytes => "write image bytes to target disk",
            FinalAction::SyncTarget => "sync target disk",
        }
    }
}

pub fn build_write_plan(image: &ImageFile, disk: &Disk) -> WritePlan {
    let decision = disk.flash_decision(Some(image.size_bytes));

    let execution_mode = match decision {
        FlashDecision::ReadyToFlash => ExecutionMode::Immediate,
        FlashDecision::NeedsUnmount => ExecutionMode::AfterPreparation,
        FlashDecision::NoImageSelected
        | FlashDecision::ImageTooLarge
        | FlashDecision::ReadOnly
        | FlashDecision::HiddenByDefault => ExecutionMode::Blocked,
    };

    let preparation_steps = if execution_mode == ExecutionMode::AfterPreparation {
        disk.mounted_filesystems()
            .into_iter()
            .map(|filesystem| PreparationStep::Unmount {
                device_path: filesystem.device_path,
                mountpoint: filesystem.mountpoint,
            })
            .collect()
    } else {
        Vec::new()
    };

    let final_actions = if execution_mode.can_eventually_write() {
        vec![FinalAction::WriteImageBytes, FinalAction::SyncTarget]
    } else {
        Vec::new()
    };

    WritePlan {
        image_path: image.path.clone(),
        image_name: image.file_name_label(),
        image_size_bytes: image.size_bytes,
        target_path: disk.path.clone(),
        target_model: disk.model_label().to_string(),
        target_size_bytes: disk.size_bytes,
        decision,
        execution_mode,
        preparation_steps,
        final_actions,
    }
}

pub fn run_preparation_steps(steps: &[PreparationStep]) -> Result<(), String> {
    for step in steps {
        match step {
            PreparationStep::Unmount { device_path, .. } => {
                let normal_result = run_umount(device_path, false);

                if normal_result.is_ok() {
                    continue;
                }

                let normal_error = normal_result.unwrap_err();

                if !normal_error.contains("target is busy") {
                    return Err(normal_error);
                }

                println!("Normal unmount failed because target is busy.");
                println!("Trying lazy unmount for {device_path}.");

                run_umount(device_path, true)?;
            }
        }
    }

    Ok(())
}

fn run_umount(device_path: &str, lazy: bool) -> Result<(), String> {
    let mut command = Command::new("umount");

    if lazy {
        command.arg("--lazy");
    }

    let output = command
        .arg(device_path)
        .output()
        .map_err(|error| format!("failed to run umount: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    if lazy {
        Err(format!(
            "failed to lazy-unmount {device_path}: {}",
            stderr.trim()
        ))
    } else {
        Err(format!(
            "failed to unmount {device_path}: {}",
            stderr.trim()
        ))
    }
}

pub fn write_image_to_file(
    image: &ImageFile,
    output_path: impl AsRef<Path>,
    on_progress: impl FnMut(WriteProgress),
) -> Result<(), String> {
    let output_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output_path.as_ref())
        .map_err(|error| format!("failed to open output file: {error}"))?;

    stream_image_to_file_handle(image, output_file, on_progress)
}

pub fn write_image_to_device(
    image: &ImageFile,
    device_path: impl AsRef<Path>,
    on_progress: impl FnMut(WriteProgress),
) -> Result<(), String> {
    let device_path = device_path.as_ref();

    if !device_path.starts_with("/dev") {
        return Err("target device must be under /dev".to_string());
    }

    let output_file = OpenOptions::new()
        .write(true)
        .open(device_path)
        .map_err(|error| format!("failed to open target device: {error}"))?;

    stream_image_to_file_handle(image, output_file, on_progress)
}

pub fn verify_image_against_device(
    image: &ImageFile,
    device_path: impl AsRef<Path>,
    mut on_progress: impl FnMut(VerifyProgress),
) -> Result<(), String> {
    let device_path = device_path.as_ref();

    if !device_path.starts_with("/dev") {
        return Err("target device must be under /dev".to_string());
    }

    let image_file =
        File::open(&image.path).map_err(|error| format!("failed to open image: {error}"))?;

    let device_file = File::open(device_path)
        .map_err(|error| format!("failed to open target device: {error}"))?;

    let mut image_reader = BufReader::new(image_file);
    let mut device_reader = BufReader::new(device_file);

    let mut image_buffer = vec![0_u8; COPY_BUFFER_SIZE];
    let mut device_buffer = vec![0_u8; COPY_BUFFER_SIZE];

    let mut bytes_checked = 0_u64;

    while bytes_checked < image.size_bytes {
        let remaining = image.size_bytes - bytes_checked;
        let chunk_size = remaining.min(COPY_BUFFER_SIZE as u64) as usize;

        image_reader
            .read_exact(&mut image_buffer[..chunk_size])
            .map_err(|error| format!("failed to read image for verification: {error}"))?;

        device_reader
            .read_exact(&mut device_buffer[..chunk_size])
            .map_err(|error| format!("failed to read target for verification: {error}"))?;

        if image_buffer[..chunk_size] != device_buffer[..chunk_size] {
            let mismatch_index = image_buffer[..chunk_size]
                .iter()
                .zip(&device_buffer[..chunk_size])
                .position(|(image_byte, device_byte)| image_byte != device_byte)
                .unwrap_or(0);

            let mismatch_offset = bytes_checked + mismatch_index as u64;

            return Err(format!(
                "verification mismatch at byte offset {mismatch_offset}"
            ));
        }

        bytes_checked += chunk_size as u64;

        on_progress(VerifyProgress {
            bytes_checked,
            total_bytes: image.size_bytes,
        });
    }

    Ok(())
}

fn stream_image_to_file_handle(
    image: &ImageFile,
    output_file: File,
    mut on_progress: impl FnMut(WriteProgress),
) -> Result<(), String> {
    let input_file =
        File::open(&image.path).map_err(|error| format!("failed to open image: {error}"))?;

    let mut reader = BufReader::new(input_file);
    let mut writer = BufWriter::new(output_file);
    let mut buffer = vec![0_u8; COPY_BUFFER_SIZE];
    let mut bytes_written = 0_u64;

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to read image: {error}"))?;

        if bytes_read == 0 {
            break;
        }

        writer
            .write_all(&buffer[..bytes_read])
            .map_err(|error| format!("failed to write target: {error}"))?;

        bytes_written += bytes_read as u64;

        on_progress(WriteProgress {
            bytes_written,
            total_bytes: image.size_bytes,
        });
    }

    writer
        .flush()
        .map_err(|error| format!("failed to flush target: {error}"))?;

    writer
        .get_ref()
        .sync_all()
        .map_err(|error| format!("failed to sync target: {error}"))?;

    Ok(())
}
