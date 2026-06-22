use crate::devices::{Disk, FlashDecision};
use crate::images::ImageFile;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct WritePlan {
    pub image_path: PathBuf,
    pub image_name: String,
    pub image_size_bytes: u64,
    pub target_path: String,
    pub target_model: String,
    pub target_size_bytes: u64,
    pub decision: FlashDecision,
    pub preparation_steps: Vec<PreparationStep>,
    pub final_actions: Vec<FinalAction>,
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

    let preparation_steps = disk
        .mounted_filesystems()
        .into_iter()
        .map(|filesystem| PreparationStep::Unmount {
            device_path: filesystem.device_path,
            mountpoint: filesystem.mountpoint,
        })
        .collect();

    WritePlan {
        image_path: image.path.clone(),
        image_name: image.file_name_label(),
        image_size_bytes: image.size_bytes,
        target_path: disk.path.clone(),
        target_model: disk.model_label().to_string(),
        target_size_bytes: disk.size_bytes,
        decision,
        preparation_steps,
        final_actions: vec![FinalAction::WriteImageBytes, FinalAction::SyncTarget],
    }
}
