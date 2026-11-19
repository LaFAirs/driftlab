//! Automated screenshot mode (`driftlab --screenshots <dir>`) and the "save report as PNG" helper.
//! Runs the demo controller through the app and captures the window with egui's screenshot command.

use crate::app::{App, Screen};
use driftlab_core::Family;
use std::path::PathBuf;

pub struct Step {
    pub name: &'static str,
    pub family: Family,
    pub screen: Screen,
    /// Wait this many seconds before capturing.
    pub wait: f64,
    pub setup: Setup,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Setup {
    None,
    DriftResult,
    WizardCompare,
}

pub struct ShotPlan {
    pub dir: PathBuf,
    pub steps: Vec<Step>,
    pub idx: usize,
    pub step_t0: f64,
    pub requested: bool,
    pub finished: bool,
    pub applied: bool,
}

impl ShotPlan {
    pub fn new(dir: PathBuf) -> Self {
        let s = |name, family, screen, wait, setup| Step { name, family, screen, wait, setup };
        Self {
            dir,
            steps: vec![
                s("overview-ps5", Family::DualSense, Screen::Overview, 4.0, Setup::None),
                s("overview-ps4", Family::DualShock4, Screen::Overview, 3.0, Setup::None),
                s("overview-xbox", Family::Xbox, Screen::Overview, 3.0, Setup::None),
                s("diagnose-ps4", Family::DualShock4, Screen::Diagnose, 5.0, Setup::DriftResult),
                s("wizard-ps5", Family::DualSense, Screen::Wizard, 12.0, Setup::WizardCompare),
                s("tuning-xbox", Family::Xbox, Screen::Tuning, 3.0, Setup::None),
            ],
            idx: 0,
            step_t0: -1.0,
            requested: false,
            finished: false,
            applied: false,
        }
    }
}

/// Called every frame.
pub fn drive(app: &mut App, ctx: &egui::Context, now: f64) {
    handle_screenshot_events(app, ctx);
    if app.diag.want_report {
        app.diag.want_report = false;
        request_shot(app, ctx, "report".to_string());
    }
    let (idx, nsteps, finished) = match &app.shot {
        Some(p) => (p.idx, p.steps.len(), p.finished),
        None => return,
    };
    if finished {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        return;
    }
    if idx >= nsteps {
        if let Some(p) = app.shot.as_mut() {
            p.finished = true;
        }
        return;
    }
    let (family, screen, wait, setup, name) = {
        let s = &app.shot.as_ref().expect("plan").steps[idx];
        (s.family, s.screen, s.wait, s.setup, s.name)
    };
    // Switch the demo device family when needed.
    if app.dev().map(|d| d.info.family) != Some(family) {
        let started = app.shot.as_ref().is_some_and(|p| p.applied);
        if !started {
            app.start_demo_force(family, now);
            if let Some(p) = app.shot.as_mut() {
                p.applied = true;
                p.step_t0 = now;
            }
        }
        return;
    }
    if app.shot.as_ref().is_some_and(|p| p.step_t0 < 0.0) {
        app.shot.as_mut().expect("plan").step_t0 = now;
    }
    if app.screen != screen {
        app.go(screen, now);
        app.welcome = None;
    }
    let (el, requested) = {
        let p = app.shot.as_ref().expect("plan");
        (now - p.step_t0, p.requested)
    };
    if !requested && el > 0.5 {
        match setup {
            Setup::DriftResult => {
                app.diag.test_secs = 1.5;
                app.diag.request_start = true;
            }
            Setup::WizardCompare => {
                app.diag.test_secs = 1.2;
                crate::screens::wizard::script_to_compare(app, now);
            }
            Setup::None => {}
        }
        app.shot.as_mut().expect("plan").requested = true;
        return;
    }
    if requested && el > wait {
        let p = app.shot.as_mut().expect("plan");
        p.requested = false;
        p.applied = false;
        p.idx += 1;
        p.step_t0 = now + 0.2; // don't re-trigger while the screenshot is in flight
        request_shot(app, ctx, format!("shot:{name}"));
    }
}
/// Ask the backend for a screenshot; the result lands in `app.shot_queue`.
pub fn request_shot(app: &App, ctx: &egui::Context, tag: String) {
    let q = app.shot_queue.clone();
    let c2 = ctx.clone();
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
        q.lock().push((tag, img));
        c2.request_repaint();
    })));
}

fn handle_screenshot_events(app: &mut App, ctx: &egui::Context) {
    let events: Vec<(std::sync::Arc<egui::ColorImage>, Option<String>)> =
        std::mem::take(&mut *app.shot_queue.lock()).into_iter().map(|(t, i)| (i, Some(t))).collect();
    let now = ctx.input(|i| i.time);
    for (img, tag) in events {
        let [w, h] = img.size;
        let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        match tag.as_deref() {
            Some("report") => {
                let path = crate::export::report_png_path();
                // Crop to the report card if we know its rect, otherwise save the whole window.
                let ppp = ctx.pixels_per_point();
                let (x0, y0, cw, ch, data) = match app.diag.report_rect {
                    Some(r) => {
                        let x0 = ((r.left() * ppp) as usize).min(w.saturating_sub(1));
                        let y0 = ((r.top() * ppp) as usize).min(h.saturating_sub(1));
                        let cw = (((r.width() * ppp) as usize).min(w - x0)).max(1);
                        let ch = (((r.height() * ppp) as usize).min(h - y0)).max(1);
                        let mut out = Vec::with_capacity(cw * ch * 4);
                        for row in y0..y0 + ch {
                            out.extend_from_slice(&rgba[(row * w + x0) * 4..(row * w + x0 + cw) * 4]);
                        }
                        (x0, y0, cw, ch, out)
                    }
                    None => (0, 0, w, h, rgba),
                };
                let _ = (x0, y0);
                match crate::export::save_png(&path, cw as u32, ch as u32, &data) {
                    Ok(()) => app.toast(now, format!("{} {}", app.t("toast.saved"), path.display()), false),
                    Err(e) => app.toast(now, e, true),
                }
            }
            Some(t) if t.starts_with("shot:") => {
                if let Some(plan) = &app.shot {
                    let p = plan.dir.join(format!("{}.png", &t[5..]));
                    if let Err(e) = crate::export::save_png(&p, w as u32, h as u32, &rgba) {
                        tracing::error!("screenshot failed: {e}");
                    }
                }
            }
            _ => {}
        }
    }
}
