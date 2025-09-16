use chaikin::*;
use speedy2d::Window;

fn main() {
    
    let window = Window::new_centered(
        "Chaikin (speedy2d) ---> Left-click add, drag to move, Enter start/pause, C clear, Esc quit",
        (WIDTH as u32, HEIGHT as u32),
    )
    .unwrap();

    let app = App::new();

    // run the event loop (this blocks until the window is closed)
    window.run_loop(app);
}
