use crate::VERSION;
use crate::gfx::{GlobeRenderer, LookAroundCamera, OrbitCamera, SkyRenderer};
use crate::ui::Selection;
use crate::ui::icons;
use crate::ui::tabs::{SkyTabViewer, Tab, default_dock};
use crate::ui::widgets::{
    EventsFilter, GlobeLayers, OrbitCache, Scrubber, SkyLayers, SystemLayers,
};
use eframe::CreationContext;
use egui::{CentralPanel, Panel, PopupCloseBehavior, TextEdit};
use egui_dock::{DockArea, DockState, Style};
use serde::{Deserialize, Serialize};
use skynav::{
    Body, Constellation, Observer, Simulation, Star, ViewWindow, catalog, constellations,
};

const STATE_KEY: &str = "skynav_state";

pub struct SkyNav {
    sim: Simulation,
    dock: DockState<Tab>,
    globe_camera: OrbitCamera,
    system_camera: OrbitCamera,
    sky_camera: LookAroundCamera,
    stars: Vec<Star>,
    constellations: Vec<Constellation>,
    system_orbits: OrbitCache,
    sky_layers: SkyLayers,
    globe_layers: GlobeLayers,
    system_layers: SystemLayers,
    events_filter: EventsFilter,
    selection: Option<Selection>,
    last_selection: Option<Selection>,
    follow: bool,
    search: String,
}

/// The slice of state persisted across sessions (the clock starts at "now").
#[derive(Serialize, Deserialize)]
struct PersistState {
    dock: DockState<Tab>,
    observer: Observer,
    rate: f64,
    playing: bool,
    selection: Option<Selection>,
    follow: bool,
    sky_layers: SkyLayers,
    globe_layers: GlobeLayers,
    system_layers: SystemLayers,
    events_filter: EventsFilter,
    view: ViewWindow,
    globe_camera: OrbitCamera,
    system_camera: OrbitCamera,
    sky_camera: LookAroundCamera,
}

impl SkyNav {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        Self::setup_fonts(&cc.egui_ctx);

        if let Some(rs) = cc.wgpu_render_state.as_ref() {
            let mut renderer = rs.renderer.write();
            renderer.callback_resources.insert(GlobeRenderer::new(rs));
            renderer.callback_resources.insert(SkyRenderer::new(rs));
        }

        let mut app = Self {
            sim: Simulation::new(crate::util::now_epoch()),
            dock: default_dock(),
            globe_camera: OrbitCamera::default(),
            system_camera: OrbitCamera::new(0.6, 0.6, 30.0),
            sky_camera: LookAroundCamera::default(),
            stars: catalog::load_stars(),
            constellations: constellations::load(),
            system_orbits: OrbitCache::default(),
            sky_layers: SkyLayers::default(),
            globe_layers: GlobeLayers::default(),
            system_layers: SystemLayers::default(),
            events_filter: EventsFilter::default(),
            selection: None,
            last_selection: None,
            follow: false,
            search: String::new(),
        };

        if let Some(storage) = cc.storage
            && let Some(state) = eframe::get_value::<PersistState>(storage, STATE_KEY)
        {
            app.restore(state);
        }
        app
    }

    fn restore(&mut self, state: PersistState) {
        self.dock = state.dock;
        self.sim.observer = state.observer;
        self.sim.clock.rate = state.rate;
        self.sim.clock.playing = state.playing;
        self.selection = state.selection;
        self.last_selection = state.selection;
        self.follow = state.follow;
        self.sky_layers = state.sky_layers;
        self.globe_layers = state.globe_layers;
        self.system_layers = state.system_layers;
        self.events_filter = state.events_filter;
        self.sim.view = state.view;
        self.globe_camera = state.globe_camera;
        self.system_camera = state.system_camera;
        self.sky_camera = state.sky_camera;
    }

    fn setup_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        ctx.set_fonts(fonts);
    }

    /// Focus the tab if it is already open, otherwise add it to the focused leaf.
    fn open_tab(&mut self, tab: Tab) {
        if let Some(found) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(found);
        } else {
            self.dock.main_surface_mut().push_to_focused_leaf(tab);
        }
    }

    /// Smoothly aim the Sky camera at a selection. The System view deliberately
    /// keeps its viewpoint (re-orbiting it on every pick was disorienting); the
    /// pulsing highlight is enough to locate the object there.
    fn focus(&mut self, sel: Selection, ctx: &egui::Context) {
        let horizontal = match sel {
            Selection::Body(Body::Earth) => return,
            Selection::Body(body) => self.sim.observed_body(body),
            Selection::Star(i) => self.stars.get(i).and_then(|s| self.sim.observed_star(s)),
        };
        if let Some(h) = horizontal {
            self.sky_camera.look_at(h.azimuth as f32, h.altitude as f32);
            ctx.request_repaint();
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, to_open: &mut Vec<Tab>) {
        let mut chosen: Option<Selection> = None;
        Panel::top("top_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("skynav v{VERSION}"));
                ui.separator();
                ui.menu_button(format!("{} Tabs", icons::PLUS), |ui| {
                    for tab in Tab::ALL {
                        if ui.button(tab.title()).clicked() {
                            to_open.push(tab);
                            ui.close();
                        }
                    }
                });
                ui.separator();

                let resp = ui.add(
                    TextEdit::singleline(&mut self.search)
                        .desired_width(170.0)
                        .hint_text(format!("{} Search bodies & stars", icons::MAGNIFYING_GLASS)),
                );
                let popup_id = egui::Id::new("skynav_search_popup");
                if resp.has_focus() && !self.search.is_empty() {
                    egui::Popup::open_id(ui.ctx(), popup_id);
                }
                let matches = search_matches(&self.search, &self.stars);
                egui::Popup::from_response(&resp)
                    .id(popup_id)
                    .open_memory(None)
                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.set_min_width(190.0);
                        if matches.is_empty() {
                            ui.weak("No matches");
                        }
                        for (label, sel) in &matches {
                            if ui.selectable_label(false, label).clicked() {
                                chosen = Some(*sel);
                            }
                        }
                    });

                ui.separator();
                let follow_label = format!("{} Follow", icons::CROSSHAIR);
                let follow = ui
                    .add_enabled(
                        self.selection.is_some(),
                        egui::Button::selectable(self.follow, follow_label),
                    )
                    .on_hover_text("Keep the Sky view centred on the selection as time advances.");
                if follow.clicked() {
                    self.follow = !self.follow;
                }
            });
        });

        if let Some(sel) = chosen {
            self.selection = Some(sel);
            self.search.clear();
            egui::Popup::close_id(ui.ctx(), egui::Id::new("skynav_search_popup"));
        }
    }
}

impl eframe::App for SkyNav {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let dt = ui.input(|i| i.stable_dt) as f64;
        self.sim.clock.advance(dt);
        if self.sim.clock.playing {
            ui.ctx().request_repaint();
        }

        let mut to_open: Vec<Tab> = Vec::new();
        self.top_bar(ui, &mut to_open);

        Panel::bottom("scrubber").show_inside(ui, |ui| {
            ui.add_space(2.0);
            ui.add(Scrubber::new(&mut self.sim));
            ui.add_space(2.0);
        });

        CentralPanel::default().show_inside(ui, |ui| {
            let mut viewer = SkyTabViewer {
                sim: &mut self.sim,
                globe_camera: &mut self.globe_camera,
                system_camera: &mut self.system_camera,
                sky_camera: &mut self.sky_camera,
                stars: &self.stars,
                constellations: &self.constellations,
                system_orbits: &mut self.system_orbits,
                system_layers: &mut self.system_layers,
                sky_layers: &mut self.sky_layers,
                globe_layers: &mut self.globe_layers,
                events_filter: &mut self.events_filter,
                selection: &mut self.selection,
                follow: &mut self.follow,
                to_open: &mut to_open,
            };
            DockArea::new(&mut self.dock)
                .style(Style::from_egui(ui.style().as_ref()))
                .show_add_buttons(true)
                .show_add_popup(true)
                .show_leaf_collapse_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_inside(ui, &mut viewer);
        });

        for tab in to_open {
            self.open_tab(tab);
        }

        if self.selection != self.last_selection {
            if let Some(sel) = self.selection {
                self.focus(sel, ui.ctx());
            }
            self.last_selection = self.selection;
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let state = PersistState {
            dock: self.dock.clone(),
            observer: self.sim.observer,
            rate: self.sim.clock.rate,
            playing: self.sim.clock.playing,
            selection: self.selection,
            follow: self.follow,
            sky_layers: self.sky_layers,
            globe_layers: self.globe_layers,
            system_layers: self.system_layers,
            events_filter: self.events_filter.clone(),
            view: self.sim.view.clone(),
            globe_camera: self.globe_camera,
            system_camera: self.system_camera,
            sky_camera: self.sky_camera,
        };
        eframe::set_value(storage, STATE_KEY, &state);
    }
}

/// Up to eight name matches (bodies first, then stars) for the search box.
fn search_matches(query: &str, stars: &[Star]) -> Vec<(String, Selection)> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for body in Body::ALL {
        if body.name().to_lowercase().contains(&q) {
            out.push((body.name().to_string(), Selection::Body(body)));
        }
    }
    for (i, star) in stars.iter().enumerate() {
        if out.len() >= 8 {
            break;
        }
        if !star.name.is_empty() && star.name.to_lowercase().contains(&q) {
            out.push((star.name.clone(), Selection::Star(i)));
        }
    }
    out.truncate(8);
    out
}
