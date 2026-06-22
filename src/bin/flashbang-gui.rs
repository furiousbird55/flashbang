// FLASHBANG_GUI_CHOOSE_IMAGE_V1

use adw::prelude::*;
use flashbang::format::format_size;
use flashbang::images::inspect_image;
use gtk::prelude::*;
use gtk::{Align, Orientation};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

const APP_ID: &str = "io.github.furiousbird55.Flashbang";

#[derive(Debug, Default)]
struct GuiState {
    selected_image_path: Option<PathBuf>,
    selected_image_size_bytes: Option<u64>,
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
        .build();

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
        .default_width(560)
        .default_height(420)
        .build();

    window.set_content(Some(&shell));

    connect_choose_image_button(
        &choose_image_button,
        &window,
        &image_label,
        &progress_bar,
        state,
    );

    window.present();
}

fn connect_choose_image_button(
    choose_image_button: &gtk::Button,
    window: &adw::ApplicationWindow,
    image_label: &gtk::Label,
    progress_bar: &gtk::ProgressBar,
    state: Rc<RefCell<GuiState>>,
) {
    let window = window.clone();
    let image_label = image_label.clone();
    let progress_bar = progress_bar.clone();

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

                        let mut state = state.borrow_mut();
                        state.selected_image_path = Some(image.path);
                        state.selected_image_size_bytes = Some(image.size_bytes);
                    }
                    Err(error) => {
                        image_label.set_label(&format!("Image error: {error}"));
                        progress_bar.set_text(Some("Image selection failed"));

                        let mut state = state.borrow_mut();
                        state.selected_image_path = None;
                        state.selected_image_size_bytes = None;
                    }
                }
            }
        });
    });
}
