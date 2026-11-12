#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod json_mini;
mod modules;
mod os;
mod state;
mod ui;

fn main() {
    if let Err(err) = os::window::NotchWindow::run() {
        eprintln!("Notch failed to initialize: {err:?}");
    }
}
