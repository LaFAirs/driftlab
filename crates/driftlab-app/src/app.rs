//! Application state, event pump, sidebar and screen routing.

use crate::i18n::{self, Lang};
use crate::screens;
use crate::theme::{self, c32, c32a, dur, radius, space, ThemeState, Tokens};
use crate::view3d::ViewState;
use crate::widgets;
use crossbeam_channel::{Receiver, Sender};
use driftlab_core::calibration::DriftReport;
use driftlab_core::manager::{self, DeviceHandle, Event, Manager};
use driftlab_core::math::{Circularity, EdgeMap};
use driftlab_core::mock::{MockControl, MockDriver};
use driftlab_core::storage::{self, Profile, ProfileStore, Settings};
use driftlab_core::*;
use egui::{pos2, vec2, Align, CornerRadius, Layout, RichText, Sense, Stroke, Ui};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Start,
    Overview,
    Diagnose,
    Wizard,
    Tuning,
    Test,
    Backups,
    Settings,
}

pub type ShotQueue = Arc<Mutex<Vec<(String, Arc<egui::ColorImage>)>>>;

pub struct Device {
    pub info: ControllerInfo,
    pub caps: Capabilities,
    pub handle: DeviceHandle,
    pub input: InputState,
    pub trail: [VecDeque<(f64, (f32, f32))>; 2],
    pub heat: [Vec<u16>; 2],
    pub series: VecDeque<[f32; 4]>,
    pub motion: VecDeque<[f32; 6]>,
    pub added_at: f64,
    pub removed_at: Option<f64>,
    pub demo: Option<Arc<Mutex<MockControl>>>,
    pub stop: Option<Arc<AtomicBool>>,
    pub rx_count: u32,
    pub rate_hz: f32,
    pub last_rate_t: f64,
}

/// Collects raw samples while a timed test is running.
pub struct Collector {
    pub start: f64,
    pub secs: f32,
    pub left: Vec<(f32, f32)>,
    pub right: Vec<(f32, f32)>,
}

pub struct Welcome {
    pub start: f64,
    pub info: ControllerInfo,
    pub battery: Option<Battery>,
}

pub struct Toast {
    pub text: String,
    pub until: f64,
    pub error: bool,
}

pub enum OpResult {
    Backup(Result<Option<CalibrationBlob>>),
    Unit(Result<()>),
    Session(Result<CalibSession>),
}

pub struct Pending {
    pub label: &'static str,
    pub rx: Receiver<OpResult>,
}

pub struct App {
    pub settings: Settings,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub manager: Manager,
    pub tx: Sender<Event>,
    pub devices: Vec<Device>,
    pub selected: Option<usize>,
    pub screen: Screen,
    pub screen_t0: f64,
    pub theme: ThemeState,
    pub tokens: Tokens,
    pub view: ViewState,
    pub welcome: Option<Welcome>,
    pub toasts: Vec<Toast>,
    pub collector: Option<Collector>,
    pub pending: Option<Pending>,
    pub profiles: ProfileStore,
    pub active_profile: usize,
    pub diag: screens::diagnose::DiagState,
    pub wizard: screens::wizard::Wizard,
    pub tuning: screens::tuning::TuningState,
    pub tester: screens::test::TestState,
    pub settings_ui: screens::settings::SettingsUi,
    pub backups_ui: screens::backups::BackupsUi,
    pub last_error: Option<String>,
    pub demo_family: Family,
    pub shot: Option<crate::shots::ShotPlan>,
    pub frame_dt: f32,
    pub last_circ: Option<(Circularity, Circularity, EdgeMap, EdgeMap)>,
    pub last_drift: Option<DriftReport>,
    pub pending_demo: Option<(Arc<Mutex<MockControl>>, Arc<AtomicBool>)>,
    pub shot_queue: ShotQueue,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, args: &crate::Args) -> Self {
        let config_dir = storage::config_dir();
        let data_dir = storage::data_dir();
        let settings = Settings::load(&config_dir);
        manager::POLL_HZ.store(settings.polling_hz, Ordering::Relaxed);
        crate::fonts::install(&cc.egui_ctx);
        let manager = Manager::start();
        let tx = manager.sender();
        let neutral = Tokens::for_key("neutral");
        let mut theme = ThemeState::new(neutral);
        theme.duration = if settings.reduced_motion { 0.0 } else { dur::THEME };
        let view = ViewState::new(cc.wgpu_render_state.clone(), args.force_2d || std::env::var_os("DRIFTLAB_2D").is_some());
        let profiles = ProfileStore::load(&data_dir);
        let mut app = Self {
            settings,
            config_dir,
            data_dir,
            manager,
            tx,
            devices: vec![],
            selected: None,
            screen: Screen::Start,
            screen_t0: 0.0,
            theme,
            tokens: neutral,
            view,
            welcome: None,
            toasts: vec![],
            collector: None,
            pending: None,
            profiles,
            active_profile: 0,
            diag: Default::default(),
            wizard: Default::default(),
            tuning: Default::default(),
            tester: Default::default(),
            settings_ui: Default::default(),
            backups_ui: Default::default(),
            last_error: None,
            demo_family: Family::DualSense,
            shot: args.screenshots.as_ref().map(|d| crate::shots::ShotPlan::new(d.clone())),
            frame_dt: 0.016,
            last_circ: None,
            last_drift: None,
            pending_demo: None,
            shot_queue: Arc::new(Mutex::new(Vec::new())),
        };
        if args.demo || app.shot.is_some() {
            app.start_demo(Family::DualSense, 0.0);
        }
        if let Some(s) = &args.screen {
            app.screen = screens::parse_screen(s);
        }
        app
    }

    pub fn lang(&self) -> Lang {
        Lang::resolve(&self.settings.language)
    }

    pub fn t(&self, key: &'static str) -> &'static str {
        i18n::tr(self.lang(), key)
    }

    pub fn reduced(&self) -> bool {
        self.settings.reduced_motion
    }

    pub fn toast(&mut self, now: f64, text: impl Into<String>, error: bool) {
        let text = text.into();
        if error {
            self.last_error = Some(text.clone());
            tracing::warn!("{text}");
        }
        self.toasts.push(Toast { text, until: now + 5.0, error });
    }

    pub fn dev(&self) -> Option<&Device> {
        self.selected.and_then(|i| self.devices.get(i))
    }

    pub fn go(&mut self, s: Screen, now: f64) {
        if self.screen != s {
            // Leaving the wizard mid-way aborts any hardware session safely.
            if self.screen == Screen::Wizard && s != Screen::Wizard {
                screens::wizard::abort_if_running(self);
            }
            self.screen = s;
            self.screen_t0 = now;
        }
    }

    pub fn start_demo(&mut self, family: Family, now: f64) {
        if self.devices.iter().any(|d| d.demo.is_some() && d.removed_at.is_none()) {
            return;
        }
        let ctl = Arc::new(Mutex::new(MockControl::default()));
        let mut drv = MockDriver::with_control(family, ctl.clone());
        drv.set_connection(Connection::Usb);
        let stop = Arc::new(AtomicBool::new(false));
        manager::spawn_device(Box::new(drv), self.tx.clone(), stop.clone());
        self.demo_family = family;
        // Remember demo handles once the Added event arrives.
        self.pending_demo = Some((ctl, stop));
        let _ = now;
    }

    /// Replace the running demo device (used by screenshot mode).
    pub fn start_demo_force(&mut self, family: Family, now: f64) {
        for d in &mut self.devices {
            if let Some(s) = &d.stop {
                s.store(true, Ordering::Relaxed);
            }
            if d.demo.is_some() && d.removed_at.is_none() {
                d.removed_at = Some(now - 1.0); // drop immediately
            }
        }
        self.pending_demo = None;
        self.start_demo(family, now);
    }

    /// Run a hardware job on the selected device, delivering the result through `pending`.
    pub fn run_job<T: Send + 'static>(
        &mut self,
        label: &'static str,
        wrap: fn(T) -> OpResult,
        f: impl FnOnce(&mut dyn ControllerDriver) -> T + Send + 'static,
    ) {
        if let Some(d) = self.dev() {
            let rx = d.handle.exec(f);
            let (tx, mrx) = crossbeam_channel::bounded(1);
            std::thread::spawn(move || {
                if let Ok(v) = rx.recv() {
                    let _ = tx.send(wrap(v));
                }
            });
            self.pending = Some(Pending { label, rx: mrx });
        }
    }

    fn pump_events(&mut self, now: f64) {
        let mut guard = 0;
        while let Ok(ev) = self.manager.events.try_recv() {
            guard += 1;
            if guard > 4000 {
                break;
            }
            match ev {
                Event::Added(info, caps, handle) => {
                    if self.devices.iter().any(|d| d.info.id == info.id && d.removed_at.is_none()) {
                        continue;
                    }
                    let demo = if info.id.starts_with("mock-") { self.pending_demo.take() } else { None };
                    let (ctl, stop) = match demo {
                        Some((c, s)) => (Some(c), Some(s)),
                        None => (None, None),
                    };
                    tracing::info!("controller added: {} ({:?})", info.name, info.connection);
                    self.devices.push(Device {
                        info: info.clone(),
                        caps,
                        handle,
                        input: InputState::default(),
                        trail: Default::default(),
                        heat: [vec![0; widgets::HEAT_N * widgets::HEAT_N], vec![0; widgets::HEAT_N * widgets::HEAT_N]],
                        series: VecDeque::new(),
                        motion: VecDeque::new(),
                        added_at: now,
                        removed_at: None,
                        demo: ctl,
                        stop,
                        rx_count: 0,
                        rate_hz: 0.0,
                        last_rate_t: now,
                    });
                    let idx = self.devices.len() - 1;
                    self.selected = Some(idx);
                    self.welcome = Some(Welcome { start: now, info, battery: None });
                    if self.screen == Screen::Start {
                        self.go(Screen::Overview, now);
                    }
                    self.view.reset_for_new_device(now);
                }
                Event::Removed(id) => {
                    if let Some(d) = self.devices.iter_mut().find(|d| d.info.id == id && d.removed_at.is_none()) {
                        d.removed_at = Some(now);
                        tracing::info!("controller removed: {}", d.info.name);
                    }
                    if self.selected.and_then(|i| self.devices.get(i)).is_some_and(|d| d.info.id == id) {
                        screens::wizard::on_disconnect(self);
                        self.collector = None;
                    }
                }
                Event::Input(id, st) => {
                    let Some(i) = self.devices.iter().position(|d| d.info.id == id && d.removed_at.is_none()) else { continue };
                    let is_sel = self.selected == Some(i);
                    let d = &mut self.devices[i];
                    d.rx_count += 1;
                    for (k, p) in [st.left, st.right].into_iter().enumerate() {
                        d.trail[k].push_back((now, p));
                        while d.trail[k].front().is_some_and(|f| now - f.0 > 2.0) {
                            d.trail[k].pop_front();
                        }
                        let n = widgets::HEAT_N;
                        let gx = (((p.0.clamp(-1.0, 1.0) + 1.0) / 2.0) * (n as f32 - 1.0)) as usize;
                        let gy = (((1.0 - p.1.clamp(-1.0, 1.0)) / 2.0) * (n as f32 - 1.0)) as usize;
                        let cell = &mut d.heat[k][gy.min(n - 1) * n + gx.min(n - 1)];
                        *cell = cell.saturating_add(1);
                    }
                    if d.rx_count % 4 == 0 {
                        d.series.push_back([st.left.0, st.left.1, st.right.0, st.right.1]);
                        if d.series.len() > 600 {
                            d.series.pop_front();
                        }
                        if let (Some(g), Some(a)) = (st.gyro, st.accel) {
                            d.motion.push_back([g[0], g[1], g[2], a[0], a[1], a[2]]);
                            if d.motion.len() > 300 {
                                d.motion.pop_front();
                            }
                        }
                    }
                    if now - d.last_rate_t >= 1.0 {
                        d.rate_hz = d.rx_count as f32 / (now - d.last_rate_t) as f32;
                        d.rx_count = 0;
                        d.last_rate_t = now;
                    }
                    if is_sel {
                        if let Some(c) = &mut self.collector {
                            c.left.push(st.left);
                            c.right.push(st.right);
                        }
                        screens::wizard::on_input(self, &st);
                        screens::diagnose::on_input(self, &st);
                    }
                    let d = &mut self.devices[i];
                    if let (Some(w), true) = (&mut self.welcome, is_sel) {
                        w.battery = st.battery;
                    }
                    d.input = *st;
                }
                Event::Error(id, msg) => self.toast(now, format!("{id}: {msg}"), true),
            }
        }
        // Drop faded-out devices.
        let sel_id = self.dev().map(|d| d.info.id.clone());
        self.devices.retain(|d| d.removed_at.is_none_or(|t| now - t < 0.5));
        self.selected = sel_id
            .and_then(|id| self.devices.iter().position(|d| d.info.id == id && d.removed_at.is_none()))
            .or_else(|| self.devices.iter().position(|d| d.removed_at.is_none()));
        if self.selected.is_none() && !matches!(self.screen, Screen::Start | Screen::Settings | Screen::Backups) {
            self.go(Screen::Start, now);
        }
        // Poll pending hardware op.
        if let Some(p) = &self.pending {
            if let Ok(r) = p.rx.try_recv() {
                let label = p.label;
                self.pending = None;
                screens::wizard::on_op_result(self, label, r, now);
            }
        }
        self.toasts.retain(|t| t.until > now);
    }

    fn update_theme(&mut self, ctx: &egui::Context, now: f64) {
        let key = match (&self.settings.theme[..], self.dev()) {
            ("auto", Some(d)) => d.info.family.theme_key(),
            ("auto", None) => "neutral",
            (m, _) => m,
        };
        self.theme.duration = if self.reduced() { 0.0 } else { dur::THEME };
        self.theme.set_target(now, Tokens::for_key(key));
        self.tokens = self.theme.current(now);
        theme::apply(ctx, &self.tokens, self.settings.ui_scale);
        if self.theme.animating(now) {
            ctx.request_repaint();
        }
    }

    fn background(&self, ui: &mut Ui, now: f64) {
        let rect = ui.max_rect();
        let t = &self.tokens;
        let mut mesh = egui::Mesh::default();
        let (top, bot) = (c32(t.bg_top), c32(t.bg_bottom));
        mesh.colored_vertex(rect.left_top(), top);
        mesh.colored_vertex(rect.right_top(), top);
        mesh.colored_vertex(rect.right_bottom(), bot);
        mesh.colored_vertex(rect.left_bottom(), bot);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        ui.painter().add(egui::Shape::mesh(mesh));
        if t.glow > 0.0 {
            let tm = if self.reduced() { 0.0 } else { now as f32 };
            for (i, (fx, fy, r)) in [(0.75f32, 0.25f32, 360.0f32), (0.2, 0.85, 300.0)].into_iter().enumerate() {
                let ph = tm * 0.15 + i as f32 * 2.0;
                let c = pos2(rect.left() + rect.width() * (fx + 0.05 * ph.sin()), rect.top() + rect.height() * (fy + 0.05 * ph.cos()));
                for k in 0..8 {
                    let f = 1.0 - k as f32 / 8.0;
                    ui.painter().circle_filled(c, r * f, c32a(t.accent, (4.0 * t.glow * (1.0 - f + 0.2)) as u8));
                }
            }
            if !self.reduced() {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
            }
        }
    }

    fn sidebar(&mut self, ui: &mut Ui, now: f64) {
        let t = self.tokens;
        ui.add_space(space::L);
        ui.horizontal(|ui| {
            ui.add_space(space::L);
            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            crate::logo::paint(ui.painter(), r, t.accent, t.sidebar_text);
            ui.label(RichText::new("DriftLab").size(20.0).strong().color(c32(t.sidebar_text)));
        });
        ui.add_space(space::XL);
        ui.horizontal(|ui| {
            ui.add_space(space::L);
            ui.label(RichText::new(self.t("nav.controllers")).size(12.0).color(c32(theme::lerp_rgb(t.sidebar_text, t.sidebar, 0.4))));
        });
        let mut click: Option<usize> = None;
        for (i, d) in self.devices.iter().enumerate() {
            let appear = if self.reduced() { 1.0 } else { theme::ease_out_cubic(((now - d.added_at) / 0.4) as f32) };
            let fade = d.removed_at.map_or(1.0, |r| 1.0 - theme::ease_in_out(((now - r) / 0.45) as f32));
            let a = (appear * fade).clamp(0.0, 1.0);
            if a <= 0.01 {
                continue;
            }
            ui.horizontal(|ui| {
                ui.add_space(space::M);
                let w = ui.available_width() - space::M;
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 52.0), Sense::click());
                let rect = rect.translate(vec2((1.0 - a) * -24.0, 0.0));
                let sel = self.selected == Some(i);
                let hov = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() || sel, dur::FAST);
                ui.painter().rect_filled(rect, CornerRadius::same(radius::M), c32a(t.accent, (hov * 40.0 * a) as u8));
                if sel {
                    ui.painter().rect_stroke(
                        rect,
                        CornerRadius::same(radius::M),
                        Stroke::new(1.5, c32a(t.accent, (200.0 * a) as u8)),
                        egui::StrokeKind::Inside,
                    );
                }
                crate::view3d::mini_icon(
                    ui.painter(),
                    egui::Rect::from_center_size(rect.left_center() + vec2(26.0, 0.0), vec2(36.0, 26.0)),
                    d.info.family,
                    &t,
                    a,
                );
                let col = c32a(t.sidebar_text, (255.0 * a) as u8);
                ui.painter().text(
                    rect.left_top() + vec2(52.0, 10.0),
                    egui::Align2::LEFT_TOP,
                    short_name(&d.info.name),
                    egui::FontId::proportional(13.5),
                    col,
                );
                let conn = match d.info.connection {
                    Connection::Usb => "USB",
                    Connection::Bluetooth => "Bluetooth",
                    Connection::Virtual => "Demo",
                    Connection::Unknown => "—",
                };
                let sub = match d.input.battery {
                    Some(b) => format!("{conn} · {} %", b.percent),
                    None => conn.to_string(),
                };
                ui.painter().text(
                    rect.left_top() + vec2(52.0, 29.0),
                    egui::Align2::LEFT_TOP,
                    sub,
                    egui::FontId::proportional(12.0),
                    c32a(theme::lerp_rgb(t.sidebar_text, t.sidebar, 0.35), (255.0 * a) as u8),
                );
                if resp.clicked() && d.removed_at.is_none() {
                    click = Some(i);
                }
            });
        }
        if self.devices.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(space::L);
                ui.label(RichText::new(self.t("nav.none")).size(13.0).color(c32(theme::lerp_rgb(t.sidebar_text, t.sidebar, 0.35))));
            });
        }
        if let Some(i) = click {
            self.selected = Some(i);
            if self.screen == Screen::Start {
                self.go(Screen::Overview, now);
            }
        }
        ui.add_space(space::XL);
        let has = self.selected.is_some();
        let items = [
            (Screen::Overview, egui_phosphor::regular::HOUSE, "nav.overview", true),
            (Screen::Diagnose, egui_phosphor::regular::PULSE, "nav.diagnose", true),
            (Screen::Wizard, egui_phosphor::regular::SLIDERS_HORIZONTAL, "nav.wizard", true),
            (Screen::Tuning, egui_phosphor::regular::SLIDERS, "nav.tuning", true),
            (Screen::Test, egui_phosphor::regular::GAME_CONTROLLER, "nav.test", true),
            (Screen::Backups, egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE, "nav.backups", false),
            (Screen::Settings, egui_phosphor::regular::GEAR, "nav.settings", false),
        ];
        for (s, icon, key, needs) in items {
            let enabled = has || !needs;
            ui.horizontal(|ui| {
                ui.add_space(space::M);
                let w = ui.available_width() - space::M;
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), if enabled { Sense::click() } else { Sense::hover() });
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, self.t(key)));
                let active = self.screen == s;
                let hov = ui.ctx().animate_bool_with_time(resp.id, (resp.hovered() && enabled) || active, dur::FAST);
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(radius::S),
                    c32a(t.accent, (hov * if active { 46.0 } else { 24.0 }) as u8),
                );
                let col = if enabled { c32(t.sidebar_text) } else { c32a(t.sidebar_text, 90) };
                ui.painter().text(
                    rect.left_center() + vec2(14.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    icon,
                    egui::FontId::proportional(18.0),
                    if active { c32(t.accent) } else { col },
                );
                ui.painter().text(
                    rect.left_center() + vec2(44.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    self.t(key),
                    egui::FontId::proportional(14.5),
                    col,
                );
                if resp.has_focus() {
                    ui.painter().rect_stroke(
                        rect,
                        CornerRadius::same(radius::S),
                        Stroke::new(2.0, c32(t.accent)),
                        egui::StrokeKind::Outside,
                    );
                }
                if resp.clicked() {
                    self.go(s, now);
                }
            });
        }
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(space::L);
            ui.horizontal(|ui| {
                ui.add_space(space::L);
                ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(12.0).color(c32(theme::lerp_rgb(
                    t.sidebar_text,
                    t.sidebar,
                    0.4,
                ))));
            });
        });
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        let t = self.tokens;
        let mut y = 16.0;
        for (i, toast) in self.toasts.iter().enumerate() {
            egui::Area::new(egui::Id::new(("toast", i))).anchor(egui::Align2::RIGHT_TOP, vec2(-16.0, y)).order(egui::Order::Tooltip).show(
                ctx,
                |ui| {
                    egui::Frame::new()
                        .fill(c32(t.card))
                        .stroke(Stroke::new(1.5, if toast.error { c32(t.bad) } else { c32(t.accent) }))
                        .corner_radius(CornerRadius::same(radius::M))
                        .inner_margin(egui::Margin::same(14))
                        .show(ui, |ui| {
                            ui.set_max_width(380.0);
                            ui.label(&toast.text);
                        });
                },
            );
            y += 64.0;
        }
        if self.toasts.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

pub fn short_name(n: &str) -> String {
    let s = n.replace(" Wireless Controller", "").replace(" Controller", "");
    if s.chars().count() > 22 {
        s.chars().take(21).collect::<String>() + "…"
    } else {
        s
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.frame_dt = ctx.input(|i| i.unstable_dt).clamp(0.001, 0.1);
        self.pump_events(now);
        self.update_theme(ctx, now);
        crate::shots::drive(self, ctx, now);
        screens::drive_demo(self, now);
        screens::diagnose::tick(self, now);
        screens::wizard::tick(self, now);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let now = ctx.input(|i| i.time);
        let t = self.tokens;
        egui::Panel::left("sidebar")
            .exact_size(244.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(c32(t.sidebar)).stroke(Stroke::new(1.0, c32(t.border))))
            .show(root, |ui| self.sidebar(ui, now));
        egui::CentralPanel::default().frame(egui::Frame::new()).show(root, |ui| {
            self.background(ui, now);
            let k = if self.reduced() { 1.0 } else { theme::ease_out_cubic(((now - self.screen_t0) as f32) / dur::SCREEN) };
            if k < 1.0 {
                ctx.request_repaint();
            }
            ui.set_opacity(k);
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                ui.add_space(space::XXL + (1.0 - k) * 20.0);
                egui::Frame::new().inner_margin(egui::Margin::symmetric(space::XXL as i8, 0)).show(ui, |ui| {
                    match self.screen {
                        Screen::Start => screens::start::ui(self, ui, now),
                        Screen::Overview => screens::overview::ui(self, ui, now),
                        Screen::Diagnose => screens::diagnose::ui(self, ui, now),
                        Screen::Wizard => screens::wizard::ui(self, ui, now),
                        Screen::Tuning => screens::tuning::ui(self, ui, now),
                        Screen::Test => screens::test::ui(self, ui, now),
                        Screen::Backups => screens::backups::ui(self, ui, now),
                        Screen::Settings => screens::settings::ui(self, ui, now),
                    }
                    ui.add_space(space::XXL);
                });
            });
        });
        self.toasts_ui(&ctx);
        // Keep live views at display refresh while a device is streaming.
        if self.devices.iter().any(|d| d.removed_at.is_none()) {
            ctx.request_repaint();
        }
    }
    fn on_exit(&mut self) {
        let _ = self.settings.save(&self.config_dir);
        let _ = self.profiles.save(&self.data_dir);
        for d in &self.devices {
            if let Some(s) = &d.stop {
                s.store(true, Ordering::Relaxed);
            }
        }
        self.manager.stop();
    }
}

pub fn active_profile_mut(app: &mut App) -> Option<&mut Profile> {
    let serial = app.dev()?.info.serial.clone().unwrap_or_else(|| "unknown".into());
    let list = app.profiles.by_serial.entry(serial).or_default();
    if list.is_empty() {
        list.push(Profile::default());
    }
    let i = app.active_profile.min(list.len() - 1);
    list.get_mut(i)
}
