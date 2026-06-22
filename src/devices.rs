use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Disk {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub model: Option<String>,
    pub transport: Option<String>,
    pub removable: bool,
    pub read_only: bool,
    pub mountpoints: Vec<String>,
    pub children: Vec<DiskChild>,
}

#[derive(Debug, Clone)]
pub struct DiskChild {
    pub name: String,
    pub path: String,
    pub device_type: String,
    pub size_bytes: u64,
    pub mountpoints: Vec<String>,
    pub children: Vec<DiskChild>,
}

#[derive(Debug, Clone)]
pub struct MountedFilesystem {
    pub device_path: String,
    pub mountpoint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashDecision {
    ReadyToFlash,
    NeedsUnmount,
    NoImageSelected,
    ImageTooLarge,
    ReadOnly,
    HiddenByDefault,
}

impl FlashDecision {
    pub fn label(&self) -> &'static str {
        match self {
            FlashDecision::ReadyToFlash => "ready to flash",
            FlashDecision::NeedsUnmount => "needs unmount first",
            FlashDecision::NoImageSelected => "no image selected",
            FlashDecision::ImageTooLarge => "image too large",
            FlashDecision::ReadOnly => "read-only",
            FlashDecision::HiddenByDefault => "hidden by default",
        }
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            FlashDecision::ReadyToFlash => "Flash",
            FlashDecision::NeedsUnmount => "Prepare & Flash",
            FlashDecision::NoImageSelected => "Choose image",
            FlashDecision::ImageTooLarge => "Choose smaller image",
            FlashDecision::ReadOnly => "Cannot flash",
            FlashDecision::HiddenByDefault => "Hidden",
        }
    }
}

impl Disk {
    pub fn flash_decision(&self, image_size_bytes: Option<u64>) -> FlashDecision {
        if !self.removable {
            return FlashDecision::HiddenByDefault;
        }

        if self.read_only {
            return FlashDecision::ReadOnly;
        }

        let Some(image_size_bytes) = image_size_bytes else {
            return FlashDecision::NoImageSelected;
        };

        if image_size_bytes > self.size_bytes {
            return FlashDecision::ImageTooLarge;
        }

        if self.has_mounts() {
            return FlashDecision::NeedsUnmount;
        }

        FlashDecision::ReadyToFlash
    }

    pub fn size_gib(&self) -> f64 {
        self.size_bytes as f64 / 1024.0 / 1024.0 / 1024.0
    }

    pub fn model_label(&self) -> &str {
        self.model.as_deref().unwrap_or("unknown model")
    }

    pub fn transport_label(&self) -> &str {
        self.transport.as_deref().unwrap_or("unknown transport")
    }

    pub fn has_mounts(&self) -> bool {
        !self.mounted_filesystems().is_empty()
    }

    pub fn mounted_filesystems(&self) -> Vec<MountedFilesystem> {
        let mut mounted = Vec::new();

        for mountpoint in &self.mountpoints {
            mounted.push(MountedFilesystem {
                device_path: self.path.clone(),
                mountpoint: mountpoint.clone(),
            });
        }

        for child in &self.children {
            child.collect_mounted_filesystems(&mut mounted);
        }

        mounted
    }
}

impl DiskChild {
    pub fn size_gib(&self) -> f64 {
        self.size_bytes as f64 / 1024.0 / 1024.0 / 1024.0
    }

    fn collect_mounted_filesystems(&self, mounted: &mut Vec<MountedFilesystem>) {
        for mountpoint in &self.mountpoints {
            mounted.push(MountedFilesystem {
                device_path: self.path.clone(),
                mountpoint: mountpoint.clone(),
            });
        }

        for child in &self.children {
            child.collect_mounted_filesystems(mounted);
        }
    }
}

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize)]
struct LsblkDevice {
    name: String,
    path: Option<String>,
    rm: Option<bool>,
    size: Option<u64>,
    ro: Option<bool>,
    model: Option<String>,
    tran: Option<String>,
    mountpoints: Option<Vec<Option<String>>>,
    children: Option<Vec<LsblkDevice>>,

    #[serde(rename = "type")]
    device_type: String,
}

pub fn discover_disks() -> Result<Vec<Disk>, String> {
    let output = Command::new("lsblk")
        .args([
            "--json",
            "--bytes",
            "--output",
            "NAME,PATH,SIZE,RM,RO,TYPE,MODEL,TRAN,MOUNTPOINTS",
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
        .map(convert_disk)
        .collect();

    Ok(disks)
}

fn convert_disk(device: LsblkDevice) -> Disk {
    let name = device.name;
    let path = device.path.unwrap_or_else(|| format!("/dev/{name}"));

    Disk {
        name,
        path,
        size_bytes: device.size.unwrap_or(0),
        model: clean_optional_text(device.model),
        transport: clean_optional_text(device.tran),
        removable: device.rm.unwrap_or(false),
        read_only: device.ro.unwrap_or(true),
        mountpoints: clean_mountpoints(device.mountpoints),
        children: device
            .children
            .unwrap_or_default()
            .into_iter()
            .map(convert_child)
            .collect(),
    }
}

fn convert_child(device: LsblkDevice) -> DiskChild {
    let name = device.name;
    let path = device.path.unwrap_or_else(|| format!("/dev/{name}"));

    DiskChild {
        name,
        path,
        device_type: device.device_type,
        size_bytes: device.size.unwrap_or(0),
        mountpoints: clean_mountpoints(device.mountpoints),
        children: device
            .children
            .unwrap_or_default()
            .into_iter()
            .map(convert_child)
            .collect(),
    }
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn clean_mountpoints(value: Option<Vec<Option<String>>>) -> Vec<String> {
    value
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect()
}
