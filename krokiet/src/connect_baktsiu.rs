use std::path::{Path, MAIN_SEPARATOR};
use std::process::Command;

use log::error;
use rfd::FileDialog;
use slint::{ComponentHandle, Model, ModelRc};

use crate::connect_rfd::{hide_file_dialog_overlay, show_file_dialog_overlay};
use crate::flk;
use crate::{ActiveTab, Callabler, MainWindow, Settings, SingleMainListModel};

pub(crate) fn connect_baktsiu(app: &MainWindow) {
    connect_open_in_baktsiu(app);
    connect_pick_baktsiu_executable(app);
}

fn connect_open_in_baktsiu(app: &MainWindow) {
    let weak = app.as_weak();
    app.global::<Callabler>().on_row_open_in_baktsiu(move |idx| {
        let app = weak.upgrade().expect("MainWindow dropped while callback is still live");
        let baktsiu_exe = app.global::<Settings>().get_baktsiu_executable_path().to_string();
        if baktsiu_exe.trim().is_empty() {
            show_baktsiu_message(
                &app,
                &flk!("baktsiu_error_executable_not_configured_title"),
                &flk!("baktsiu_error_executable_not_configured_message"),
            );
            return;
        }
        if !Path::new(&baktsiu_exe).is_file() {
            show_baktsiu_message(
                &app,
                &flk!("baktsiu_error_executable_not_found_title"),
                &flk!("baktsiu_error_executable_not_found_message", path = baktsiu_exe),
            );
            return;
        }

        let active_tab = app.global::<crate::GuiState>().get_active_tab();
        if matches!(active_tab, ActiveTab::Settings | ActiveTab::About) {
            return;
        }

        let model = active_tab.get_tool_model(&app);
        let row_idx = idx as usize;
        let paths = collect_paths_for_baktsiu(&model, active_tab, row_idx);
        if paths.is_empty() {
            show_baktsiu_message(
                &app,
                &flk!("baktsiu_error_no_files_title"),
                &flk!("baktsiu_error_no_files_message"),
            );
            return;
        }

        if let Err(reason) = launch_baktsiu(&baktsiu_exe, &paths) {
            error!("Failed to launch Baktsiu: {reason}");
            show_baktsiu_message(
                &app,
                &flk!("baktsiu_error_launch_failed_title"),
                &flk!("baktsiu_error_launch_failed_message", reason = reason),
            );
        }
    });
}

fn connect_pick_baktsiu_executable(app: &MainWindow) {
    let weak = app.as_weak();
    app.global::<Callabler>().on_pick_baktsiu_executable(move || {
        let app = weak.upgrade().expect("MainWindow dropped while callback is still live");
        let weak_overlay = app.as_weak();
        show_file_dialog_overlay(&app);
        let picked = FileDialog::new().pick_file();
        hide_file_dialog_overlay(&weak_overlay);
        if let Some(path) = picked {
            app.global::<Settings>()
                .set_baktsiu_executable_path(path.to_string_lossy().to_string().into());
        }
    });
}

fn show_baktsiu_message(app: &MainWindow, title: &str, message: &str) {
    let _ = app;
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Warning)
        .show();
}

pub(crate) fn launch_baktsiu(baktsiu_exe: &str, paths: &[String]) -> Result<(), String> {
    let mut command = Command::new(baktsiu_exe);
    command.arg("--columns");
    for path in paths {
        command.arg(path);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn collect_paths_for_baktsiu(model: &ModelRc<SingleMainListModel>, active_tab: ActiveTab, row_idx: usize) -> Vec<String> {
    if row_idx >= model.row_count() {
        return Vec::new();
    }

    let path_idx = active_tab.get_str_path_idx();
    let name_idx = active_tab.get_str_name_idx();

    if active_tab.get_is_header_mode() {
        let header_idx = find_group_header_index(model, row_idx);
        let mut paths = Vec::new();
        let mut i = header_idx + 1;
        while i < model.row_count() {
            let row = model.row_data(i).expect("row index must be within model bounds");
            if row.header_row {
                break;
            }
            if let Some(full_path) = row_full_path(&row, path_idx, name_idx) {
                if Path::new(&full_path).is_file() {
                    paths.push(full_path);
                }
            }
            i += 1;
        }
        paths
    } else {
        let row = model.row_data(row_idx).expect("row index must be within model bounds");
        if row.header_row {
            return Vec::new();
        }
        row_full_path(&row, path_idx, name_idx)
            .filter(|p| Path::new(p).is_file())
            .into_iter()
            .collect()
    }
}

fn find_group_header_index(model: &ModelRc<SingleMainListModel>, row_idx: usize) -> usize {
    let row = model.row_data(row_idx).expect("row index must be within model bounds");
    if row.header_row {
        return row_idx;
    }
    for i in (0..=row_idx).rev() {
        if model.row_data(i).expect("index must be within row_count").header_row {
            return i;
        }
    }
    model
        .iter()
        .enumerate()
        .find(|(_, r)| r.header_row)
        .map_or(0, |(i, _)| i)
}

fn row_full_path(row: &SingleMainListModel, path_idx: usize, name_idx: usize) -> Option<String> {
    let path = row.val_str.iter().nth(path_idx)?.to_string();
    let name = row.val_str.iter().nth(name_idx)?.to_string();
    if name.is_empty() && path.is_empty() {
        return None;
    }
    if name.is_empty() {
        Some(path)
    } else if path.is_empty() {
        Some(name)
    } else {
        Some(format!("{path}{MAIN_SEPARATOR}{name}"))
    }
}
