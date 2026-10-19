#![forbid(unsafe_code)]

mod app;
mod chrome;
mod editor;
mod i18n;
mod input;
mod library;
mod library_io;
mod screens;
mod store;
mod titlebar;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    let mut app = app::App::new();
    event_loop.run_app(&mut app).expect("run app");
}
