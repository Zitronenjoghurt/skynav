use crate::gfx::{LookAroundCamera, OrbitCamera, UnifiedCamera};
use crate::ui::widgets::{
    BodiesPanel, ChecklistPanel, EventsFilter, EventsPanel, GlobeLayers, GlobeView, InfoPanel,
    ObserverPanel, OrbitCache, SkyLayers, SkyView, SystemLayers, SystemView, ViewPanel,
    VisiblePanel,
};
use crate::ui::{Observed, Selection, icons};
use egui_dock::{DockState, NodeIndex, NodePath, TabViewer};
use skynav::{Constellation, Simulation, Star};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Tab {
    Globe,
    System,
    Sky,
    Info,
    Visible,
    Checklist,
    Observer,
    Bodies,
    Events,
    View,
}

impl Tab {
    /// Tabs grouped by purpose, for a friendlier "open a tab" menu.
    pub const GROUPS: &'static [(&'static str, &'static [Tab])] = &[
        ("3D views", &[Tab::Globe, Tab::System, Tab::Sky]),
        (
            "Information",
            &[
                Tab::Info,
                Tab::Visible,
                Tab::Bodies,
                Tab::Events,
                Tab::Checklist,
            ],
        ),
        ("Setup", &[Tab::Observer, Tab::View]),
    ];

    /// One-line description shown as a tooltip in the open-a-tab menu, so users
    /// can tell at a glance what each tab is for.
    pub fn blurb(&self) -> &'static str {
        match self {
            Tab::Globe => "Fly from a planet's surface out to the whole solar system.",
            Tab::System => "Top-down orrery of the planets on their orbits.",
            Tab::Sky => "First-person planetarium of the sky above the observer.",
            Tab::Info => "Detailed facts about the selected body or star.",
            Tab::Visible => "Everything currently above the horizon, highest first.",
            Tab::Checklist => "Track which objects you have personally observed.",
            Tab::Observer => "Set the body and location you are observing from.",
            Tab::Bodies => "Table of every Solar System body with live data.",
            Tab::Events => "Upcoming and past eclipses, conjunctions, rises and sets.",
            Tab::View => "Limit what counts as visible to patches of your sky.",
        }
    }

    pub fn title(&self) -> String {
        let (icon, label) = match self {
            Tab::Globe => (icons::GLOBE_HEMISPHERE_WEST, "Explorer"),
            Tab::System => (icons::ATOM, "System"),
            Tab::Sky => (icons::STAR, "Sky"),
            Tab::Info => (icons::INFO, "Info"),
            Tab::Visible => (icons::EYE, "Visible"),
            Tab::Checklist => (icons::LIST_CHECKS, "Checklist"),
            Tab::Observer => (icons::MAP_PIN, "Observer"),
            Tab::Bodies => (icons::PLANET, "Bodies"),
            Tab::Events => (icons::CALENDAR_BLANK, "Events"),
            Tab::View => (icons::BINOCULARS, "View"),
        };
        format!("{icon} {label}")
    }
}

/// Default workspace: the Explorer (with the System orrery beside it) on the
/// left, control panels docked right. The old first-person Sky view is folded
/// into the Explorer's surface mode, so it is no longer a default tab (still
/// openable from the "+" menu).
pub fn default_dock() -> DockState<Tab> {
    // Big Explorer (with System and Sky as tabs) on the left; the right side is a
    // three-row column: Info/Checklist on top, the Bodies table in the middle,
    // and a bottom row split into Visible/Events and Observer/View.
    let mut dock = DockState::new(vec![Tab::Globe, Tab::System, Tab::Sky]);
    let surface = dock.main_surface_mut();
    let [_, top] = surface.split_right(NodeIndex::root(), 0.64, vec![Tab::Info]);
    let [top, middle] = surface.split_below(top, 0.32, vec![Tab::Bodies]);
    let [_, bottom] = surface.split_below(middle, 0.46, vec![Tab::Visible, Tab::Events]);
    surface.split_right(bottom, 0.5, vec![Tab::Observer, Tab::View]);
    // Top row split side by side: Info on the left, Checklist on the right.
    surface.split_right(top, 0.5, vec![Tab::Checklist]);
    dock
}

pub struct SkyTabViewer<'a> {
    pub sim: &'a mut Simulation,
    pub globe_camera: &'a mut UnifiedCamera,
    pub system_camera: &'a mut OrbitCamera,
    pub sky_camera: &'a mut LookAroundCamera,
    pub stars: &'a [Star],
    pub constellations: &'a [Constellation],
    pub system_orbits: &'a mut OrbitCache,
    pub system_layers: &'a mut SystemLayers,
    pub sky_layers: &'a mut SkyLayers,
    pub globe_layers: &'a mut GlobeLayers,
    pub events_filter: &'a mut EventsFilter,
    /// Object selected across the Sky / System / Bodies views (shared highlight).
    pub selection: &'a mut Option<Selection>,
    /// Objects the user has marked as observed (shared with the checklist).
    pub observed: &'a mut Observed,
    /// Sky follow-cam: keep the selection centred as time advances.
    pub follow: &'a mut bool,
    /// Tabs requested via the dock "+" button, opened by the app after the frame.
    pub to_open: &'a mut Vec<Tab>,
}

impl TabViewer for SkyTabViewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::Globe => {
                ui.add(GlobeView::new(
                    self.sim,
                    self.globe_camera,
                    self.stars,
                    self.constellations,
                    self.selection,
                    self.globe_layers,
                    self.system_orbits,
                ));
            }
            Tab::System => {
                ui.add(SystemView::new(
                    self.sim,
                    self.system_camera,
                    self.system_orbits,
                    self.selection,
                    self.system_layers,
                ));
            }
            Tab::Sky => {
                ui.add(SkyView::new(
                    self.sim,
                    self.sky_camera,
                    self.stars,
                    self.constellations,
                    self.selection,
                    self.sky_layers,
                    self.follow,
                ));
            }
            Tab::Info => {
                ui.add(InfoPanel::new(
                    self.sim,
                    self.stars,
                    *self.selection,
                    self.observed,
                ));
            }
            Tab::Visible => {
                ui.add(VisiblePanel::new(
                    self.sim,
                    self.stars,
                    self.selection,
                    self.observed,
                ));
            }
            Tab::Checklist => {
                ui.add(ChecklistPanel::new(
                    self.stars,
                    self.constellations,
                    self.observed,
                    self.selection,
                ));
            }
            Tab::Observer => {
                ui.add(ObserverPanel::new(self.sim));
            }
            Tab::Bodies => {
                ui.add(BodiesPanel::new(self.sim, self.selection, self.observed));
            }
            Tab::Events => {
                ui.add(EventsPanel::new(self.sim, self.events_filter));
            }
            Tab::View => {
                ui.add(ViewPanel::new(self.sim, self.sky_camera));
            }
        }
    }

    fn add_popup(&mut self, ui: &mut egui::Ui, _node: NodePath) {
        ui.set_min_width(170.0);
        for (group, tabs) in Tab::GROUPS {
            ui.label(egui::RichText::new(*group).small().weak());
            for tab in *tabs {
                if ui.button(tab.title()).on_hover_text(tab.blurb()).clicked() {
                    self.to_open.push(*tab);
                }
            }
            ui.add_space(2.0);
        }
    }
}
