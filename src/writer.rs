use crate::devices::{Disk, FlashDecision};
use crate::images::ImageFile;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

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

impl WriteProgress {
    pub fn percent(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.bytes_written as f64 / self.total_bytes as f64) * 100.0
        }
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

pub fn write_image_to_file(
    image: &ImageFile,
    output_path: impl AsRef<Path>,
    mut on_progress: impl FnMut(WriteProgress),
) -> Result<(), String> {
    let input_file =
        File::open(&image.path).map_err(|error| format!("failed to open image: {error}"))?;

    let output_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output_path.as_ref())
        .map_err(|error| format!("failed to open output file: {error}"))?;

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
            .map_err(|error| format!("failed to write output file: {error}"))?;

        bytes_written += bytes_read as u64;

        on_progress(WriteProgress {
            bytes_written,
            total_bytes: image.size_bytes,
        });
    }

    writer
        .flush()
        .map_err(|error| format!("failed to flush output file: {error}"))?;

    writer
        .get_ref()
        .sync_all()
        .map_err(|error| format!("failed to sync output file: {error}"))?;

    Ok(())
}
