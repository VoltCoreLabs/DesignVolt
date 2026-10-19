mod anim;
mod app;
mod draw;
mod editor;
mod icons;
mod text;
mod theme;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    let mut app = app::App::new();
    event_loop.run_app(&mut app).expect("run app");
}
