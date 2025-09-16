use speedy2d::color::Color;
use speedy2d::dimen::Vector2;
use speedy2d::window::{MouseButton, VirtualKeyCode, WindowHandler, WindowHelper};
use speedy2d::Graphics2D;
use std::time::{Duration, Instant};

pub const WIDTH: f32 = 1200.0;
pub const HEIGHT: f32 = 900.0;
const MAX_STEPS: usize = 7;
const CLICK_RADIUS: f32 = 10.0; // also used as closed-detection threshold
const POINT_OUTER_R: f32 = 7.0;
const POINT_INNER_R: f32 = 3.0;
const ANIM_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug)]
struct Pt {
    x: f32,
    y: f32,
}

impl From<Vector2<f32>> for Pt {
    fn from(v: Vector2<f32>) -> Self {
        Pt { x: v.x, y: v.y }
    }
}

impl Into<Vector2<f32>> for Pt {
    fn into(self) -> Vector2<f32> {
        Vector2::new(self.x, self.y)
    }
}

fn dist2(a: Pt, b: Pt) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

/// Chaikin step: supports both open (endpoint-preserving) and closed (cyclic) variants.
/// - If `closed == false`: preserves first and last control points and inserts Q/R for interior segments.
/// - If `closed == true`: treats the polygon as closed and wraps indices modulo n => produces 2n points.
fn chaikin_step(points: &[Pt], closed: bool) -> Vec<Pt> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }

    if closed {
        // Closed (cyclic) variant: for each segment i -> (i+1)%n produce Q and R
        let mut out = Vec::with_capacity(n * 2);
        for i in 0..n {
            let p0 = points[i];
            let p1 = points[(i + 1) % n];
            let q = Pt {
                x: p0.x * 0.75 + p1.x * 0.25,
                y: p0.y * 0.75 + p1.y * 0.25,
            };
            let r = Pt {
                x: p0.x * 0.25 + p1.x * 0.75,
                y: p0.y * 0.25 + p1.y * 0.75,
            };
            out.push(q);
            out.push(r);
        }
        out
    } else {
        // Open endpoint-preserving variant
        if n == 2 {
            // For two points, keep them as-is (Chaikin would produce 2 points too, but we prefer a straight line)
            return points.to_vec();
        }

        let mut out = Vec::with_capacity(n * 2 + 2);
        out.push(points[0]); // preserve first endpoint
        for i in 0..(n - 1) {
            let p0 = points[i];
            let p1 = points[i + 1];
            let q = Pt {
                x: p0.x * 0.75 + p1.x * 0.25,
                y: p0.y * 0.75 + p1.y * 0.25,
            };
            let r = Pt {
                x: p0.x * 0.25 + p1.x * 0.75,
                y: p0.y * 0.25 + p1.y * 0.75,
            };
            out.push(q);
            out.push(r);
        }
        out.push(points[n - 1]); // preserve last endpoint
        out
    }
}

/// Precompute iterations up to `max_steps`. Detect & normalize closed polygons:
/// - if `closed == true` or if first and last are very close we treat it as closed.
/// - if a closed polygon contains duplicated final point (first==last) we remove the duplicate before iterating.
fn precompute_iterations(base: &[Pt], max_steps: usize, closed_hint: bool) -> Vec<Vec<Pt>> {
    // decide closed-ness: explicit hint OR auto-detect by first/last proximity
    let mut closed = closed_hint;
    if base.len() >= 3 {
        let first = base[0];
        let last = base[base.len() - 1];
        if dist2(first, last) <= CLICK_RADIUS * CLICK_RADIUS {
            closed = true;
        }
    }

    // normalize base for closed polygons: drop duplicate last point if present
    let mut cur: Vec<Pt> = if closed && base.len() >= 2 {
        // if last and first are identical (or very close), drop last duplicate
        if dist2(base[0], base[base.len() - 1]) <= CLICK_RADIUS * CLICK_RADIUS {
            base[0..base.len() - 1].to_vec()
        } else {
            base.to_vec()
        }
    } else {
        base.to_vec()
    };

    let mut iters: Vec<Vec<Pt>> = Vec::with_capacity(max_steps + 1);
    iters.push(cur.clone()); // step 0

    for _ in 0..max_steps {
        if cur.len() < 2 {
            iters.push(cur.clone());
        } else {
            cur = chaikin_step(&cur, closed);
            iters.push(cur.clone());
        }
    }

    iters
}

pub struct App {
    control_points: Vec<Pt>,
    cached_iters: Vec<Vec<Pt>>,
    dragging: Option<usize>,
    last_mouse_pos: Vector2<f32>,
    anim_running: bool,
    anim_step: usize,
    last_anim_instant: Instant,
}

impl App {
    pub fn new() -> App {
        let ctrl = Vec::new();
        let cached = precompute_iterations(&ctrl, MAX_STEPS, false);
        App {
            control_points: ctrl,
            cached_iters: cached,
            dragging: None,
            last_mouse_pos: Vector2::new(0.0, 0.0),
            anim_running: false,
            anim_step: 0,
            last_anim_instant: Instant::now(),
        }
    }

    fn mouse_pos_to_pt(pos: Vector2<f32>) -> Pt {
        Pt {
            x: pos.x.clamp(0.0, WIDTH),
            y: pos.y.clamp(0.0, HEIGHT),
        }
    }

    fn find_point_index_near(&self, pt: Pt, radius: f32) -> Option<usize> {
        let r2 = radius * radius;
        for (i, p) in self.control_points.iter().enumerate() {
            let dx = p.x - pt.x;
            let dy = p.y - pt.y;
            if dx * dx + dy * dy <= r2 {
                return Some(i);
            }
        }
        None
    }

    /// Helper to recompute cached iterations using auto-detection of closed/open.
    fn recompute_cache(&mut self) {
        // pass `false` hint; precompute_iterations will auto-detect by spatial closeness
        self.cached_iters = precompute_iterations(&self.control_points, MAX_STEPS, false);
        // ensure anim_step within bounds
        if self.anim_step >= self.cached_iters.len() {
            self.anim_step = 0;
        }
    }
}

impl WindowHandler for App {
    fn on_start(&mut self, _helper: &mut WindowHelper, _info: speedy2d::window::WindowStartupInfo) {}

    fn on_draw(&mut self, helper: &mut WindowHelper, graphics: &mut Graphics2D) {
        // update animation timing
        if self.anim_running && self.control_points.len() >= 3 {
            if self.last_anim_instant.elapsed() >= ANIM_INTERVAL {
                self.last_anim_instant = Instant::now();
                self.anim_step = self.anim_step.wrapping_add(1);
                if self.anim_step > MAX_STEPS {
                    self.anim_step = 0;
                }
            }
        } else {
            self.anim_step = 0;
        }

        // choose dataset to draw
        // For closed polygons, cached_iters[anim_step] already represents the normalized closed polygon (no duplicated final pt).
        let to_draw: &[Pt] = if self.control_points.len() >= 3 {
            &self.cached_iters[self.anim_step]
        } else {
            &self.control_points
        };

        // Clear background
        graphics.clear_screen(Color::from_rgb(0.07, 0.07, 0.07));

        // If the polygon is closed (detected), draw faint closed control polygon as context.
        let closed_detected = if self.control_points.len() >= 3 {
            dist2(self.control_points[0], self.control_points[self.control_points.len() - 1])
                <= CLICK_RADIUS * CLICK_RADIUS
        } else {
            false
        };

        if !closed_detected {
            // draw faint original control polyline for context (when >=3)
            if self.control_points.len() >= 3 {
                for i in 0..(self.control_points.len() - 1) {
                    let a: Vector2<f32> =
                        Vector2::new(self.control_points[i].x, self.control_points[i].y);
                    let b: Vector2<f32> = Vector2::new(
                        self.control_points[i + 1].x,
                        self.control_points[i + 1].y,
                    );
                    self.draw_line(graphics,a, b, 1.0, false);
        }
            }
        } else {
            // Draw faint closed polygon connecting last->first as well
            if self.control_points.len() >= 3 {
                for i in 0..(self.control_points.len() - 1) {
                    let a: Vector2<f32> =
                        Vector2::new(self.control_points[i].x, self.control_points[i].y);
                    let b: Vector2<f32> = Vector2::new(
                        self.control_points[i + 1].x,
                        self.control_points[i + 1].y,
                    );
                    self.draw_line(graphics,a, b, 1.0, false);
                }
                // close the loop
                let a = Vector2::new(self.control_points[self.control_points.len() - 1].x,
                                     self.control_points[self.control_points.len() - 1].y);
                let b = Vector2::new(self.control_points[0].x, self.control_points[0].y);
                self.draw_line(graphics,a, b, 1.0, false);
            }
        }

        // draw the active polyline (curve)
        if to_draw.len() >= 2 {
            for i in 0..(to_draw.len() - 1) {
                let a: Vector2<f32> = Vector2::new(to_draw[i].x, to_draw[i].y);
                let b: Vector2<f32> = Vector2::new(to_draw[i + 1].x, to_draw[i + 1].y);
                    self.draw_line(graphics,a, b, 2.0, true);
            }
            // if closed, connect last -> first to visualize it (cached_iters for closed is cyclic points)
            if closed_detected {
                let last = to_draw[to_draw.len() - 1];
                let first = to_draw[0];
                let a: Vector2<f32> = Vector2::new(last.x, last.y);
                let b: Vector2<f32> = Vector2::new(first.x, first.y);
                self.draw_line(graphics,a, b, 2.0, true);
            }
        }

        // draw control points (outer bright circle + inner dark)
        for p in &self.control_points {
            let center = Vector2::new(p.x, p.y);
            graphics.draw_circle(center, POINT_OUTER_R, Color::WHITE);
            graphics.draw_circle(center, POINT_INNER_R, Color::from_rgb(0.12, 0.12, 0.12));
        }

        // request redraw so we get smooth animation & interactive dragging
        helper.request_redraw();
    }

    fn on_mouse_move(&mut self, _helper: &mut WindowHelper, position: Vector2<f32>) {
        self.last_mouse_pos = position;
        if let Some(idx) = self.dragging {
            if idx < self.control_points.len() {
                let p = App::mouse_pos_to_pt(position);
                self.control_points[idx] = p;
                self.recompute_cache();
            } else {
                self.dragging = None;
            }
        }
    }

    
    // NOTE: mouse button callbacks no longer receive position — use `self.last_mouse_pos`
    fn on_mouse_button_down(&mut self, _helper: &mut WindowHelper, button: MouseButton) {
        if button == MouseButton::Left {
            let pt = App::mouse_pos_to_pt(self.last_mouse_pos);
            if let Some(idx) = self.find_point_index_near(pt, CLICK_RADIUS) {
                // start dragging existing point
                self.dragging = Some(idx);
            } else {
                // add new point
                self.control_points.push(pt);
                self.recompute_cache();
            }
        }
    }
    
    fn on_mouse_button_up(&mut self, _helper: &mut WindowHelper, button: MouseButton) {
        if button == MouseButton::Left {
            self.dragging = None;
        }
    }
    
    // key callbacks now include scancode (u32)
    fn on_key_down(
        &mut self,
        _helper: &mut WindowHelper,
        virtual_key_code: Option<VirtualKeyCode>,
        _scancode: u32,
    ) {
        if let Some(key) = virtual_key_code {
            match key {
                VirtualKeyCode::Escape => {
                    // quit
                    std::process::exit(0);
                }
                VirtualKeyCode::Return | VirtualKeyCode::NumpadEnter => {
                    // Toggle animation only when there are points
                    if self.control_points.len() == 0 {
                        // do nothing
                    } else {
                        self.anim_running = !self.anim_running;
                        if self.anim_running {
                            self.anim_step = 0;
                            self.last_anim_instant = Instant::now();
                        }
                    }
                }
                VirtualKeyCode::C => {
                    // clear
                    self.control_points.clear();
                    self.recompute_cache();
                    self.anim_running = false;
                    self.anim_step = 0;
                }
                _ => {}
            }
        }
    }
    
    // implement other handlers as no-op
    fn on_key_up(
        &mut self,
        _helper: &mut WindowHelper,
        _virtual_key_code: Option<VirtualKeyCode>,
        _scancode: u32,
    ) {
    }
    
    fn on_resize(&mut self, _helper: &mut WindowHelper, _size_pixels: Vector2<u32>) {}
}

impl App {
    fn draw_line(&self ,graphics: &mut Graphics2D, a: Vector2<f32>, b: Vector2<f32>,t:f32,c :bool) {
        if self.anim_running {
            if c {
                graphics.draw_line(a, b, t, Color::WHITE);
            } else {
                graphics.draw_line(a, b, t, Color::from_rgb(0.12, 0.12, 0.12));
            }
        }
        
    }
}

//********************************************************************************
/*
use speedy2d::color::Color;
use speedy2d::dimen::Vector2;
use speedy2d::window::{
    MouseButton, VirtualKeyCode, WindowHandler, WindowHelper
    };
    use speedy2d::Graphics2D;
    use std::time::{Duration, Instant};
    
    pub const WIDTH: f32 = 1400.0;
    pub const HEIGHT: f32 = 920.0;
    const MAX_STEPS: usize = 7;
    const CLICK_RADIUS: f32 = 10.0;
    const POINT_OUTER_R: f32 = 7.0;
    const POINT_INNER_R: f32 = 3.0;
    const ANIM_INTERVAL: Duration = Duration::from_millis(500);
    
    #[derive(Clone, Copy, Debug)]
    struct Pt {
        x: f32,
        y: f32,
        }
        
        impl From<Vector2<f32>> for Pt {
            fn from(v: Vector2<f32>) -> Self {
                Pt { x: v.x, y: v.y }
                }
            }
            
            impl Into<Vector2<f32>> for Pt {
                fn into(self) -> Vector2<f32> {
                    Vector2::new(self.x, self.y)
                    }
                }
                
                // fn chaikin_step(points: &[Pt]) -> Vec<Pt> {
                    //     if points.len() < 2 {
//         return points.to_vec();
//     }
//     let mut out = Vec::with_capacity(points.len() * 2);
//     for i in 0..(points.len() - 1) {
//         let p0 = points[i];
//         let p1 = points[i + 1];
//         let q = Pt {
//             x: p0.x * 0.75 + p1.x * 0.25,
//             y: p0.y * 0.75 + p1.y * 0.25,
//         };
//         let r = Pt {
//             x: p0.x * 0.25 + p1.x * 0.75,
//             y: p0.y * 0.25 + p1.y * 0.75,
//         };
//         out.push(q);
//         out.push(r);
//     }
//     out
// }

fn chaikin_step(points: &[Pt]) -> Vec<Pt> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(n * 2 + 2);
    out.push(points[0]); // preserve start point

    for i in 0..(n - 1) {
        let p0 = points[i];
        let p1 = points[i + 1];

        // standard Chaikin weights
        let q = Pt {
            x: p0.x * 0.75 + p1.x * 0.25,
            y: p0.y * 0.75 + p1.y * 0.25,
        };
        let r = Pt {
            x: p0.x * 0.25 + p1.x * 0.75,
            y: p0.y * 0.25 + p1.y * 0.75,
        };

        out.push(q);
        out.push(r);
    }

    out.push(points[n - 1]); // preserve end point
    out
}


fn precompute_iterations(base: &[Pt], max_steps: usize) -> Vec<Vec<Pt>> {
    let mut iters: Vec<Vec<Pt>> = Vec::with_capacity(max_steps + 1);
    iters.push(base.to_vec());
    let mut cur = base.to_vec();
    for _ in 0..max_steps {
        if cur.len() < 2 {
            iters.push(cur.clone());
        } else {
            cur = chaikin_step(&cur);
            iters.push(cur.clone());
        }
    }
    iters
}

pub struct App {
    control_points: Vec<Pt>,
    cached_iters: Vec<Vec<Pt>>,
    dragging: Option<usize>,
    last_mouse_pos: Vector2<f32>,
    anim_running: bool,
    anim_step: usize,
    last_anim_instant: Instant,
}

impl App {
    pub fn new() -> App {
        let ctrl = Vec::new();
        let cached = precompute_iterations(&ctrl, MAX_STEPS);
        App {
            control_points: ctrl,
            cached_iters: cached,
            dragging: None,
            last_mouse_pos: Vector2::new(0.0, 0.0),
            anim_running: false,
            anim_step: 0,
            last_anim_instant: Instant::now(),
        }
    }

    fn mouse_pos_to_pt(pos: Vector2<f32>) -> Pt {
        Pt {
            x: pos.x.clamp(0.0, WIDTH),
            y: pos.y.clamp(0.0, HEIGHT),
        }
    }

    fn find_point_index_near(&self, pt: Pt, radius: f32) -> Option<usize> {
        let r2 = radius * radius;
        for (i, p) in self.control_points.iter().enumerate() {
            let dx = p.x - pt.x;
            let dy = p.y - pt.y;
            if dx * dx + dy * dy <= r2 {
                return Some(i);
            }
        }
        None
    }
}

impl WindowHandler for App {
    // fn on_start(&mut self, _helper: &mut WindowHelper, _info: speedy2d::window::WindowStartupInfo) {
    //     // nothing special at start
    // }

    fn on_draw(&mut self, helper: &mut WindowHelper, graphics: &mut Graphics2D) {
        // update animation timing
        if self.anim_running && self.control_points.len() >= 3 {
            if self.last_anim_instant.elapsed() >= ANIM_INTERVAL {
                self.last_anim_instant = Instant::now();
                self.anim_step = self.anim_step.wrapping_add(1);
                if self.anim_step > MAX_STEPS {
                    self.anim_step = 0;
                }
            }
        } else {
            // animation irrelevant for <3 points
            self.anim_step = 0;
        }

        // choose dataset to draw
        let to_draw: &[Pt] = if self.control_points.len() >= 3 {
            &self.cached_iters[self.anim_step]
        } else {
            &self.control_points
        };

        // Clear background
        graphics.clear_screen(Color::from_rgb(0.07, 0.07, 0.07));

        // draw faint original control polyline for context (when >=3)
        if self.control_points.len() >= 3 {
            for i in 0..(self.control_points.len() - 1) {
                let a: Vector2<f32> = Vector2::new(self.control_points[i].x, self.control_points[i].y);
                let b: Vector2<f32> =
                    Vector2::new(self.control_points[i + 1].x, self.control_points[i + 1].y);
                graphics.draw_line(a, b, 2.0, Color::from_rgb(0.3,0.3,0.3));
            }
        }

        // draw the active polyline (curve)
        if to_draw.len() >= 2 {
            for i in 0..(to_draw.len() - 1) {
                let a: Vector2<f32> = Vector2::new(to_draw[i].x, to_draw[i].y);
                let b: Vector2<f32> = Vector2::new(to_draw[i + 1].x, to_draw[i + 1].y);
                graphics.draw_line(a, b, 2.0, Color::GREEN);
            }
        }

        // special-case exactly 2 control points: draw a stronger straight line
        if self.control_points.len() == 2 {
            let a = Vector2::new(self.control_points[0].x, self.control_points[0].y);
            let b = Vector2::new(self.control_points[1].x, self.control_points[1].y);
            graphics.draw_line(a, b, 3.0, Color::GREEN);
        }

        // draw control points (outer bright circle + inner dark)
        for p in &self.control_points {
            let center = Vector2::new(p.x, p.y);
            graphics.draw_circle(center, POINT_OUTER_R, Color::RED);
            graphics.draw_circle(center, POINT_INNER_R, Color::from_rgb(0.12, 0.12, 0.12));
        }

        // request redraw so we get smooth animation & interactive dragging
        helper.request_redraw();
    }

    fn on_mouse_move(&mut self, _helper: &mut WindowHelper, position: Vector2<f32>) {
        self.last_mouse_pos = position;
        if let Some(idx) = self.dragging {
            if idx < self.control_points.len() {
                let p = App::mouse_pos_to_pt(position);
                self.control_points[idx] = p;
                self.cached_iters = precompute_iterations(&self.control_points, MAX_STEPS);
            } else {
                self.dragging = None;
            }
        }
    }

    // NOTE: mouse button callbacks no longer receive position — use `self.last_mouse_pos`
    fn on_mouse_button_down(&mut self, _helper: &mut WindowHelper, button: MouseButton) {
        if button == MouseButton::Left {
            let pt = App::mouse_pos_to_pt(self.last_mouse_pos);
            if let Some(idx) = self.find_point_index_near(pt, CLICK_RADIUS) {
                // start dragging existing point
                self.dragging = Some(idx);
            } else {
                // add new point
                self.control_points.push(pt);
                self.cached_iters = precompute_iterations(&self.control_points, MAX_STEPS);
            }
        }
    }

    fn on_mouse_button_up(&mut self, _helper: &mut WindowHelper, button: MouseButton) {
        if button == MouseButton::Left {
            self.dragging = None;
        }
    }

    // key callbacks now include scancode (u32)
    fn on_key_down(&mut self, _helper: &mut WindowHelper, virtual_key_code: Option<VirtualKeyCode>, _scancode: u32) {
        if let Some(key) = virtual_key_code {
            match key {
                VirtualKeyCode::Escape => {
                    // quit
                    std::process::exit(0);
                }
                VirtualKeyCode::Return | VirtualKeyCode::NumpadEnter => {
                    // Toggle animation only when there are points
                    if self.control_points.len() == 0 {
                        // do nothing
                    } else {
                        self.anim_running = !self.anim_running;
                        if self.anim_running {
                            self.anim_step = 0;
                            self.last_anim_instant = Instant::now();
                        }
                    }
                }
                VirtualKeyCode::C => {
                    // clear
                    self.control_points.clear();
                    self.cached_iters = precompute_iterations(&self.control_points, MAX_STEPS);
                    self.anim_running = false;
                    self.anim_step = 0;
                }
                _ => {}
            }
        }
    }

    // implement other handlers as no-op
    fn on_key_up(&mut self, _helper: &mut WindowHelper, _virtual_key_code: Option<VirtualKeyCode>, _scancode: u32) {}

    fn on_resize(&mut self, _helper: &mut WindowHelper, _size_pixels: Vector2<u32>) {}
}
*/