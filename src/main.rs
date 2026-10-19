mod anim;
mod app;
mod chrome;
mod clipboard;
mod draw;
mod editor;
mod icons;
mod input;
mod screens;
mod store;
mod text;
mod theme;
mod titlebar;
mod widgets;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    let mut app = app::App::new();
    event_loop.run_app(&mut app).expect("run app");
}
