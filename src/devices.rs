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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskStatus {
    FlashCandidate,
    HiddenByDefault,
}

impl Disk {
    pub fn status(&self) -> DiskStatus {
        if self.removable && !self.read_only {
            DiskStatus::FlashCandidate
        } else {
            DiskStatus::HiddenByDefault
        }
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
}

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize)]
struct LsblkDevice {
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

pub fn discover_disks() -> Result<Vec<Disk>, String> {
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
        .map(|device| {
            let name = device.name;
            let path = device.path.unwrap_or_else(|| format!("/dev/{name}"));

            Disk {
                name,
                path,
                size_bytes: device.size,
                model: clean_optional_text(device.model),
                transport: clean_optional_text(device.tran),
                removable: device.rm,
                read_only: device.ro,
            }
        })
        .collect();

    Ok(disks)
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}