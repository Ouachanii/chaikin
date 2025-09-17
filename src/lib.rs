use speedy2d::{
    color::Color,
    dimen::Vector2,
    window::{MouseButton, VirtualKeyCode, WindowHandler, WindowHelper},
    Graphics2D,
};
use std::time::{Duration, Instant};

pub const WIDTH: f32 = 1024.0;
pub const HEIGHT: f32 = 860.0;
const MAX_STEPS: usize = 7;
const CLICK_RADIUS: f32 = 10.0;
const POINT_OUTER_R: f32 = 7.0;
const POINT_INNER_R: f32 = 3.0;
const ANIM_INTERVAL: Duration = Duration::from_millis(800);

#[derive(Clone, Copy, Debug)]
struct Pt {
    x: f32,
    y: f32,
}

impl From<Vector2<f32>> for Pt {
    fn from(v: Vector2<f32>) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<Pt> for Vector2<f32> {
    fn from(p: Pt) -> Self {
        Vector2::new(p.x, p.y)
    }
}

fn dist2(a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    dx * dx + dy * dy
}

fn chaikin_step(points: &[Pt], closed: bool) -> Vec<Pt> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }

    let mut out = Vec::with_capacity(n * 2 + 2);

    if closed {
        for i in 0..n {
            let (p0, p1) = (points[i], points[(i + 1) % n]);
            out.push(Pt {
                x: p0.x * 0.75 + p1.x * 0.25,
                y: p0.y * 0.75 + p1.y * 0.25,
            });
            out.push(Pt {
                x: p0.x * 0.25 + p1.x * 0.75,
                y: p0.y * 0.25 + p1.y * 0.75,
            });
        }
    } else {
        out.push(points[0]);
        for w in points.windows(2) {
            let (p0, p1) = (w[0], w[1]);
            out.push(Pt {
                x: p0.x * 0.75 + p1.x * 0.25,
                y: p0.y * 0.75 + p1.y * 0.25,
            });
            out.push(Pt {
                x: p0.x * 0.25 + p1.x * 0.75,
                y: p0.y * 0.25 + p1.y * 0.75,
            });
        }
        out.push(points[n - 1]);
    }

    out
}

fn precompute_iterations(base: &[Pt], max_steps: usize, mut closed: bool) -> Vec<Vec<Pt>> {
    if base.len() >= 3 && dist2(base[0], *base.last().unwrap()) <= CLICK_RADIUS * CLICK_RADIUS {
        closed = true;
    }

    let mut cur = if closed && base.len() >= 2 && dist2(base[0], *base.last().unwrap()) <= CLICK_RADIUS * CLICK_RADIUS {
        base[..base.len() - 1].to_vec()
    } else {
        base.to_vec()
    };

    let mut iters = Vec::with_capacity(max_steps + 1);
    iters.push(cur.clone());

    for _ in 0..max_steps {
        cur = chaikin_step(&cur, closed);
        iters.push(cur.clone());
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
    pub fn new() -> Self {
        let control_points = Vec::new();
        Self {
            cached_iters: precompute_iterations(&control_points, MAX_STEPS, false),
            control_points,
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
        self.control_points
            .iter()
            .position(|p| dist2(*p, pt) <= r2)
    }

    fn recompute_cache(&mut self) {
        self.cached_iters = precompute_iterations(&self.control_points, MAX_STEPS, false);
        if self.anim_step >= self.cached_iters.len() {
            self.anim_step = 0;
        }
    }

    fn draw_line(&self, graphics: &mut Graphics2D, a: Pt, b: Pt, thickness: f32, highlight: bool) {
    if self.anim_running {
        let color = if highlight {
            Color::GREEN
        } else {
            Color::from_rgb(0.12, 0.12, 0.12)
        };

        let a: Vector2<f32> = a.into();
        let b: Vector2<f32> = b.into();
        graphics.draw_line(a, b, thickness, color);
    }
}

}

impl WindowHandler for App {
    fn on_draw(&mut self, helper: &mut WindowHelper, graphics: &mut Graphics2D) {
        if self.anim_running && self.control_points.len() >= 3 && self.last_anim_instant.elapsed() >= ANIM_INTERVAL {
            self.last_anim_instant = Instant::now();
            self.anim_step = (self.anim_step + 1) % (MAX_STEPS + 1);
        } else if !self.anim_running {
            self.anim_step = 0;
        }

        let to_draw = if self.control_points.len() >= 3 {
            &self.cached_iters[self.anim_step]
        } else {
            &self.control_points
        };

        graphics.clear_screen(Color::from_rgb(0.07, 0.07, 0.07));

        let closed_detected = self.control_points.len() >= 3
            && dist2(self.control_points[0], *self.control_points.last().unwrap())
                <= CLICK_RADIUS * CLICK_RADIUS;

        if self.control_points.len() >= 3 {
            let iter = self.control_points.windows(2);
            for w in iter {
                self.draw_line(graphics, w[0], w[1], 1.0, false);
            }
            if closed_detected {
                self.draw_line(graphics, *self.control_points.last().unwrap(), self.control_points[0], 1.0, false);
            }
        }

        if to_draw.len() >= 2 {
            for w in to_draw.windows(2) {
                self.draw_line(graphics, w[0], w[1], 2.0, true);
            }
            if closed_detected {
                self.draw_line(graphics, *to_draw.last().unwrap(), to_draw[0], 2.0, true);
            }
        }

        for p in &self.control_points {
            let center: Vector2<f32> = (*p).into();
            graphics.draw_circle(center, POINT_OUTER_R, Color::RED);
            graphics.draw_circle(center, POINT_INNER_R, Color::from_rgb(0.12, 0.12, 0.12));
        }

        helper.request_redraw();
    }

    fn on_mouse_move(&mut self, _helper: &mut WindowHelper, position: Vector2<f32>) {
        self.last_mouse_pos = position;
        if let Some(idx) = self.dragging {
            if let Some(p) = self.control_points.get_mut(idx) {
                *p = Self::mouse_pos_to_pt(position);
                self.recompute_cache();
            } else {
                self.dragging = None;
            }
        }
    }

    fn on_mouse_button_down(&mut self, _helper: &mut WindowHelper, button: MouseButton) {
        let pt = Self::mouse_pos_to_pt(self.last_mouse_pos);
        match button {
            MouseButton::Right => self.dragging = self.find_point_index_near(pt, CLICK_RADIUS),
            MouseButton::Left => {
                self.control_points.push(pt);
                self.recompute_cache();
            }
            _ => {}
        }
    }

    fn on_mouse_button_up(&mut self, _helper: &mut WindowHelper, button: MouseButton) {
        if button == MouseButton::Right {
            self.dragging = None;
        }
    }

    fn on_key_down(&mut self, _helper: &mut WindowHelper, key: Option<VirtualKeyCode>, _scancode: u32) {
        match key {
            Some(VirtualKeyCode::Escape) => std::process::exit(0),
            Some(VirtualKeyCode::Return | VirtualKeyCode::NumpadEnter) if !self.control_points.is_empty() => {
                self.anim_running = !self.anim_running;
                if self.anim_running {
                    self.anim_step = 0;
                    self.last_anim_instant = Instant::now();
                }
            }
            Some(VirtualKeyCode::C) => {
                self.control_points.clear();
                self.recompute_cache();
                self.anim_running = false;
                self.anim_step = 0;
            }
            _ => {}
        }
    }
}
