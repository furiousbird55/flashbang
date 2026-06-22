// FLASHBANG_FLASH_WORKFLOW_V1

use crate::devices::{FlashDecision, discover_disks};
use crate::images::ImageFile;
use crate::writer::{
    ExecutionMode, VerifyProgress, WriteProgress, build_write_plan, run_preparation_steps,
    verify_image_against_device, write_image_to_device,
};

#[derive(Debug, Clone)]
pub struct FlashRequest {
    pub image: ImageFile,
    pub target_path: String,
    pub verify_after_write: bool,
}

#[derive(Debug, Clone)]
pub enum FlashEvent {
    PlanBuilt {
        target_path: String,
        target_model: String,
        target_size_bytes: u64,
        decision: FlashDecision,
        execution_mode: ExecutionMode,
        verify_after_write: bool,
    },
    Preparing {
        step_label: String,
    },
    PreparationFinished,
    Writing {
        progress: WriteProgress,
    },
    WriteFinished,
    Verifying {
        progress: VerifyProgress,
    },
    VerifyFinished,
    Finished,
}

pub fn flash_image_to_device(
    request: FlashRequest,
    mut on_event: impl FnMut(FlashEvent),
) -> Result<(), String> {
    let disks = discover_disks()?;

    let Some(disk) = disks.iter().find(|disk| disk.path == request.target_path) else {
        return Err(format!(
            "target device was not found by Flashbang: {}",
            request.target_path
        ));
    };

    let plan = build_write_plan(&request.image, disk);

    on_event(FlashEvent::PlanBuilt {
        target_path: plan.target_path.clone(),
        target_model: plan.target_model.clone(),
        target_size_bytes: plan.target_size_bytes,
        decision: plan.decision,
        execution_mode: plan.execution_mode,
        verify_after_write: request.verify_after_write,
    });

    if !plan.execution_mode.can_eventually_write() {
        return Err("this target is not writable in its current state".to_string());
    }

    for step in &plan.preparation_steps {
        on_event(FlashEvent::Preparing {
            step_label: step.label(),
        });

        run_preparation_steps(std::slice::from_ref(step))?;
    }

    if !plan.preparation_steps.is_empty() {
        on_event(FlashEvent::PreparationFinished);
    }

    let refreshed_disks = discover_disks()?;

    let Some(refreshed_disk) = refreshed_disks
        .iter()
        .find(|disk| disk.path == request.target_path)
    else {
        return Err(format!(
            "target disappeared after preparation: {}",
            request.target_path
        ));
    };

    let refreshed_decision = refreshed_disk.flash_decision(Some(request.image.size_bytes));

    if refreshed_decision != FlashDecision::ReadyToFlash {
        return Err(format!(
            "target is still not ready after preparation: {}",
            refreshed_decision.label()
        ));
    }

    write_image_to_device(&request.image, request.target_path.as_str(), |progress| {
        on_event(FlashEvent::Writing { progress });
    })?;

    on_event(FlashEvent::WriteFinished);

    if request.verify_after_write {
        verify_image_against_device(&request.image, request.target_path.as_str(), |progress| {
            on_event(FlashEvent::Verifying { progress });
        })?;

        on_event(FlashEvent::VerifyFinished);
    }

    on_event(FlashEvent::Finished);

    Ok(())
}
