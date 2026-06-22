// FLASHBANG_GUI_PKEXEC_FLASH_V1

use adw::prelude::*;
use flashbang::devices::{FlashDecision, discover_disks};
use flashbang::format::format_size;
use flashbang::images::inspect_image;
use gtk::glib::{self, ControlFlow};
use gtk::{Align, Orientation};
use std::cell::RefCell;
use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const APP_ID: &str = "io.github.furiousbird55.Flashbang";

#[derive(Debug, Clone)]
struct DeviceChoice {
    path: String,
    label: String,
}

#[derive(Debug, Default)]
struct GuiState {
    selected_image_path: Option<PathBuf>,
    selected_image_size_bytes: Option<u64>,
    device_choices: Vec<DeviceChoice>,
    selected_device_path: Option<String>,
}

#[derive(Debug)]
enum GuiFlashMessage {
    Status(String),
    Progress { phase: String, percent: f64 },
    Finished(Result<(), String>),
}

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let state = Rc::new(RefCell::new(GuiState::default()));

    let title = gtk::Label::builder()
        .label("Flashbang")
        .halign(Align::Start)
        .build();
    title.add_css_class("title-1");

    let subtitle = gtk::Label::builder()
        .label("A small Linux USB image flasher that does not need to be a whole circus.")
        .halign(Align::Start)
        .wrap(true)
        .build();
    subtitle.add_css_class("dim-label");

    let image_label = gtk::Label::builder()
        .label("No image selected")
        .halign(Align::Start)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();

    let choose_image_button = gtk::Button::with_label("Choose image");

    let device_label = gtk::Label::builder()
        .label("No target device selected")
        .halign(Align::Start)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();

    let refresh_devices_button = gtk::Button::with_label("Refresh devices");

    let device_dropdown = gtk::DropDown::from_strings(&["No removable devices found"]);
    device_dropdown.set_sensitive(false);
    device_dropdown.set_hexpand(true);

    let verify_checkbox = gtk::CheckButton::with_label("Verify after flashing");
    verify_checkbox.set_active(false);

    let progress_bar = gtk::ProgressBar::new();
    progress_bar.set_show_text(true);
    progress_bar.set_text(Some("Idle"));

    let flash_button = gtk::Button::with_label("Flash");
    flash_button.add_css_class("suggested-action");
    flash_button.set_sensitive(false);

    let image_row = gtk::Box::new(Orientation::Horizontal, 12);
    image_row.append(&choose_image_button);
    image_row.append(&image_label);

    let device_row = gtk::Box::new(Orientation::Horizontal, 12);
    device_row.append(&refresh_devices_button);
    device_row.append(&device_dropdown);

    let button_row = gtk::Box::new(Orientation::Horizontal, 12);
    button_row.set_halign(Align::End);
    button_row.append(&flash_button);

    let content = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    content.append(&title);
    content.append(&subtitle);
    content.append(&image_row);
    content.append(&device_row);
    content.append(&device_label);
    content.append(&verify_checkbox);
    content.append(&progress_bar);
    content.append(&button_row);

    let header = adw::HeaderBar::new();
    let window_title = adw::WindowTitle::new("Flashbang", "USB image flasher");
    header.set_title_widget(Some(&window_title));

    let shell = gtk::Box::new(Orientation::Vertical, 0);
    shell.append(&header);
    shell.append(&content);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Flashbang")
        .default_width(640)
        .default_height(460)
        .build();

    window.set_content(Some(&shell));

    connect_choose_image_button(
        &choose_image_button,
        &window,
        &image_label,
        &progress_bar,
        &flash_button,
        state.clone(),
    );

    connect_device_dropdown(
        &device_dropdown,
        &device_label,
        &flash_button,
        state.clone(),
    );

    connect_refresh_devices_button(
        &refresh_devices_button,
        &device_dropdown,
        &device_label,
        &flash_button,
        state.clone(),
    );

    connect_flash_button(
        &flash_button,
        &progress_bar,
        &verify_checkbox,
        state.clone(),
    );

    refresh_device_choices(&device_dropdown, &device_label, &flash_button, &state);

    window.present();
}

fn connect_choose_image_button(
    choose_image_button: &gtk::Button,
    window: &adw::ApplicationWindow,
    image_label: &gtk::Label,
    progress_bar: &gtk::ProgressBar,
    flash_button: &gtk::Button,
    state: Rc<RefCell<GuiState>>,
) {
    let window = window.clone();
    let image_label = image_label.clone();
    let progress_bar = progress_bar.clone();
    let flash_button = flash_button.clone();

    choose_image_button.connect_clicked(move |_| {
        let dialog = gtk::FileChooserNative::new(
            Some("Choose image"),
            Some(&window),
            gtk::FileChooserAction::Open,
            Some("Open"),
            Some("Cancel"),
        );

        let iso_filter = gtk::FileFilter::new();
        iso_filter.set_name(Some("Disk images"));
        iso_filter.add_pattern("*.iso");
        iso_filter.add_pattern("*.img");
        iso_filter.add_pattern("*.raw");
        dialog.add_filter(&iso_filter);

        let all_filter = gtk::FileFilter::new();
        all_filter.set_name(Some("All files"));
        all_filter.add_pattern("*");
        dialog.add_filter(&all_filter);

        let state = state.clone();
        let image_label = image_label.clone();
        let progress_bar = progress_bar.clone();
        let flash_button = flash_button.clone();

        dialog.run_async(move |dialog, response| {
            if response == gtk::ResponseType::Accept {
                let Some(file) = dialog.file() else {
                    return;
                };

                let Some(path) = file.path() else {
                    image_label.set_label("Could not read selected file path");
                    progress_bar.set_text(Some("Image selection failed"));
                    return;
                };

                match inspect_image(&path) {
                    Ok(image) => {
                        let label = format!(
                            "{} — {}",
                            image.file_name_label(),
                            format_size(image.size_bytes)
                        );

                        image_label.set_label(&label);
                        progress_bar.set_fraction(0.0);
                        progress_bar.set_text(Some("Image selected"));

                        {
                            let mut state = state.borrow_mut();
                            state.selected_image_path = Some(image.path.clone());
                            state.selected_image_size_bytes = Some(image.size_bytes);
                        }

                        update_flash_button(&flash_button, &state);
                    }
                    Err(error) => {
                        image_label.set_label(&format!("Image error: {error}"));
                        progress_bar.set_text(Some("Image selection failed"));

                        {
                            let mut state = state.borrow_mut();
                            state.selected_image_path = None;
                            state.selected_image_size_bytes = None;
                        }

                        update_flash_button(&flash_button, &state);
                    }
                }
            }
        });
    });
}

fn connect_device_dropdown(
    device_dropdown: &gtk::DropDown,
    device_label: &gtk::Label,
    flash_button: &gtk::Button,
    state: Rc<RefCell<GuiState>>,
) {
    let device_label = device_label.clone();
    let flash_button = flash_button.clone();

    device_dropdown.connect_selected_notify(move |dropdown| {
        let selected_index = dropdown.selected() as usize;

        {
            let mut state = state.borrow_mut();

            if let Some(choice) = state.device_choices.get(selected_index).cloned() {
                device_label.set_label(&format!("Selected target: {}", choice.path));
                state.selected_device_path = Some(choice.path);
            } else {
                device_label.set_label("No target device selected");
                state.selected_device_path = None;
            }
        }

        update_flash_button(&flash_button, &state);
    });
}

fn connect_refresh_devices_button(
    refresh_devices_button: &gtk::Button,
    device_dropdown: &gtk::DropDown,
    device_label: &gtk::Label,
    flash_button: &gtk::Button,
    state: Rc<RefCell<GuiState>>,
) {
    let device_dropdown = device_dropdown.clone();
    let device_label = device_label.clone();
    let flash_button = flash_button.clone();

    refresh_devices_button.connect_clicked(move |_| {
        refresh_device_choices(&device_dropdown, &device_label, &flash_button, &state);
    });
}

fn connect_flash_button(
    flash_button: &gtk::Button,
    progress_bar: &gtk::ProgressBar,
    verify_checkbox: &gtk::CheckButton,
    state: Rc<RefCell<GuiState>>,
) {
    let button_for_callback = flash_button.clone();
    let progress_bar_for_callback = progress_bar.clone();
    let verify_checkbox_for_callback = verify_checkbox.clone();

    flash_button.connect_clicked(move |_| {
        let (image_path, target_path) = {
            let state = state.borrow();

            let Some(image_path) = state.selected_image_path.clone() else {
                progress_bar_for_callback.set_text(Some("Choose an image first"));
                return;
            };

            let Some(target_path) = state.selected_device_path.clone() else {
                progress_bar_for_callback.set_text(Some("Choose a target device first"));
                return;
            };

            (image_path, target_path)
        };

        if let Err(error) = inspect_image(&image_path) {
            progress_bar_for_callback.set_text(Some(&format!("Image error: {error}")));
            return;
        }

        let verify_after_write = verify_checkbox_for_callback.is_active();

        button_for_callback.set_sensitive(false);
        progress_bar_for_callback.set_fraction(0.0);
        progress_bar_for_callback.set_text(Some("Waiting for authentication"));

        let (sender, receiver) = mpsc::channel::<GuiFlashMessage>();
        let finish_sender = sender.clone();

        thread::spawn(move || {
            let result = run_privileged_flash_command(
                &image_path,
                &target_path,
                verify_after_write,
                move |message| {
                    let _ = sender.send(message);
                },
            );

            let _ = finish_sender.send(GuiFlashMessage::Finished(result));
        });

        watch_flash_messages(&button_for_callback, &progress_bar_for_callback, receiver);
    });
}

fn run_privileged_flash_command(
    image_path: &Path,
    target_path: &str,
    verify_after_write: bool,
    mut send_message: impl FnMut(GuiFlashMessage),
) -> Result<(), String> {
    let cli_path = sibling_cli_binary_path()?;

    send_message(GuiFlashMessage::Status(
        "Authentication requested".to_string(),
    ));

    let mut command = Command::new("pkexec");
    command
        .arg(cli_path)
        .arg(image_path)
        .arg("--write-device")
        .arg(target_path)
        .arg("--yes")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if verify_after_write {
        command.arg("--verify");
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start pkexec: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture privileged helper stdout".to_string())?;

    let stderr = child.stderr.take();

    let stderr_thread = stderr.map(|stderr| {
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let mut collected = String::new();

            for line in reader.lines().map_while(Result::ok) {
                collected.push_str(&line);
                collected.push('\n');
            }

            collected
        })
    });

    let reader = BufReader::new(stdout);
    let mut phase = "Working".to_string();

    for line in reader.lines() {
        let line =
            line.map_err(|error| format!("failed to read privileged helper output: {error}"))?;

        if line.contains("Preparing target:") {
            phase = "Preparing".to_string();
            send_message(GuiFlashMessage::Status("Preparing target".to_string()));
        } else if line.contains("Writing image to device:") {
            phase = "Writing".to_string();
            send_message(GuiFlashMessage::Status("Writing image".to_string()));
        } else if line.contains("Verifying written image:") {
            phase = "Verifying".to_string();
            send_message(GuiFlashMessage::Status("Verifying image".to_string()));
        } else if line.contains("Device write complete.") {
            send_message(GuiFlashMessage::Status("Write complete".to_string()));
        } else if line.contains("Verification complete") {
            send_message(GuiFlashMessage::Status("Verification complete".to_string()));
        } else if line.contains("Flash operation finished.") {
            send_message(GuiFlashMessage::Status(
                "Flash operation finished".to_string(),
            ));
        }

        if let Some(percent) = parse_progress_percent(&line) {
            send_message(GuiFlashMessage::Progress {
                phase: phase.clone(),
                percent,
            });
        }
    }

    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for privileged helper: {error}"))?;

    let stderr_text = match stderr_thread {
        Some(handle) => handle
            .join()
            .unwrap_or_else(|_| "failed to read helper stderr".to_string()),
        None => String::new(),
    };

    if status.success() {
        Ok(())
    } else if stderr_text.trim().is_empty() {
        Err(format!("privileged helper exited with status {status}"))
    } else {
        Err(stderr_text.trim().to_string())
    }
}

fn sibling_cli_binary_path() -> Result<PathBuf, String> {
    let mut path =
        env::current_exe().map_err(|error| format!("failed to locate GUI executable: {error}"))?;

    path.set_file_name("flashbang");

    if !path.exists() {
        return Err(format!(
            "CLI helper was not found at {}. Run: cargo build --bin flashbang --bin flashbang-gui",
            path.display()
        ));
    }

    Ok(path)
}

fn parse_progress_percent(line: &str) -> Option<f64> {
    let after_marker = line.split("progress:").nth(1)?;
    let percent_text = after_marker.split('%').next()?.trim();
    percent_text.parse::<f64>().ok()
}

fn watch_flash_messages(
    flash_button: &gtk::Button,
    progress_bar: &gtk::ProgressBar,
    receiver: mpsc::Receiver<GuiFlashMessage>,
) {
    let flash_button = flash_button.clone();
    let progress_bar = progress_bar.clone();

    glib::timeout_add_local(Duration::from_millis(100), move || {
        let mut operation_finished = false;

        while let Ok(message) = receiver.try_recv() {
            match message {
                GuiFlashMessage::Status(status) => {
                    progress_bar.set_text(Some(&status));
                }
                GuiFlashMessage::Progress { phase, percent } => {
                    progress_bar.set_fraction(percent / 100.0);
                    progress_bar.set_text(Some(&format!("{phase} {:.0}%", percent)));
                }
                GuiFlashMessage::Finished(result) => {
                    operation_finished = true;
                    flash_button.set_sensitive(true);

                    match result {
                        Ok(()) => {
                            progress_bar.set_fraction(1.0);
                            progress_bar.set_text(Some("Finished"));
                        }
                        Err(error) => {
                            progress_bar.set_fraction(0.0);
                            progress_bar.set_text(Some(&format!("Failed: {error}")));
                        }
                    }
                }
            }
        }

        if operation_finished {
            ControlFlow::Break
        } else {
            ControlFlow::Continue
        }
    });
}

fn refresh_device_choices(
    device_dropdown: &gtk::DropDown,
    device_label: &gtk::Label,
    flash_button: &gtk::Button,
    state: &Rc<RefCell<GuiState>>,
) {
    let disks = match discover_disks() {
        Ok(disks) => disks,
        Err(error) => {
            set_device_dropdown_labels(device_dropdown, &["Device scan failed"]);
            device_dropdown.set_sensitive(false);
            device_label.set_label(&format!("Device scan failed: {error}"));

            {
                let mut state = state.borrow_mut();
                state.device_choices.clear();
                state.selected_device_path = None;
            }

            update_flash_button(flash_button, state);
            return;
        }
    };

    let choices: Vec<DeviceChoice> = disks
        .into_iter()
        .filter(|disk| disk.flash_decision(None) != FlashDecision::HiddenByDefault)
        .map(|disk| {
            let preparation = if disk.mounted_filesystems().is_empty() {
                "ready"
            } else {
                "will unmount"
            };

            let label = format!(
                "{} — {} — {} ({})",
                disk.model_label(),
                disk.path,
                format_size(disk.size_bytes),
                preparation
            );

            DeviceChoice {
                path: disk.path,
                label,
            }
        })
        .collect();

    if choices.is_empty() {
        set_device_dropdown_labels(device_dropdown, &["No removable devices found"]);
        device_dropdown.set_sensitive(false);
        device_label.set_label("No target device selected");

        {
            let mut state = state.borrow_mut();
            state.device_choices.clear();
            state.selected_device_path = None;
        }

        update_flash_button(flash_button, state);
        return;
    }

    let labels: Vec<String> = choices.iter().map(|choice| choice.label.clone()).collect();
    let label_refs: Vec<&str> = labels.iter().map(|label| label.as_str()).collect();

    set_device_dropdown_labels(device_dropdown, &label_refs);
    device_dropdown.set_sensitive(true);
    device_dropdown.set_selected(0);

    {
        let mut state = state.borrow_mut();
        state.device_choices = choices;
        state.selected_device_path = state
            .device_choices
            .first()
            .map(|choice| choice.path.clone());

        if let Some(path) = &state.selected_device_path {
            device_label.set_label(&format!("Selected target: {path}"));
        } else {
            device_label.set_label("No target device selected");
        }
    }

    update_flash_button(flash_button, state);
}

fn set_device_dropdown_labels(device_dropdown: &gtk::DropDown, labels: &[&str]) {
    let string_list = gtk::StringList::new(labels);
    device_dropdown.set_model(Some(&string_list));
}

fn update_flash_button(flash_button: &gtk::Button, state: &Rc<RefCell<GuiState>>) {
    let state = state.borrow();

    flash_button
        .set_sensitive(state.selected_image_path.is_some() && state.selected_device_path.is_some());
}
