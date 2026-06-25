use crate::gfx::{LookAroundCamera, OrbitCamera};
use crate::ui::widgets::{
    BodiesPanel, ChecklistPanel, EventsFilter, EventsPanel, GlobeLayers, GlobeView, InfoPanel,
    ObserverPanel, OrbitCache, SkyLayers, SkyView, SystemLayers, SystemView, TimePanel, ViewPanel,
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
    Time,
    Observer,
    Bodies,
    Events,
    View,
}

impl Tab {
    pub const ALL: [Tab; 11] = [
        Tab::Globe,
        Tab::System,
        Tab::Sky,
        Tab::Info,
        Tab::Visible,
        Tab::Checklist,
        Tab::Time,
        Tab::Observer,
        Tab::Bodies,
        Tab::Events,
        Tab::View,
    ];

    pub fn title(&self) -> String {
        let (icon, label) = match self {
            Tab::Globe => (icons::GLOBE_HEMISPHERE_WEST, "Globe"),
            Tab::System => (icons::ATOM, "System"),
            Tab::Sky => (icons::STAR, "Sky"),
            Tab::Info => (icons::INFO, "Info"),
            Tab::Visible => (icons::EYE, "Visible"),
            Tab::Checklist => (icons::LIST_CHECKS, "Checklist"),
            Tab::Time => (icons::CLOCK, "Time"),
            Tab::Observer => (icons::MAP_PIN, "Observer"),
            Tab::Bodies => (icons::PLANET, "Bodies"),
            Tab::Events => (icons::CALENDAR_BLANK, "Events"),
            Tab::View => (icons::BINOCULARS, "View"),
        };
        format!("{icon} {label}")
    }
}

/// Default workspace: the 3D views on the left, control panels docked right.
pub fn default_dock() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Globe, Tab::System, Tab::Sky]);
    dock.main_surface_mut().split_right(
        NodeIndex::root(),
        0.72,
        vec![
            Tab::Info,
            Tab::Visible,
            Tab::Checklist,
            Tab::Time,
            Tab::Observer,
            Tab::Bodies,
            Tab::Events,
            Tab::View,
        ],
    );
    dock
}

pub struct SkyTabViewer<'a> {
    pub sim: &'a mut Simulation,
    pub globe_camera: &'a mut OrbitCamera,
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
                    self.selection,
                    self.globe_layers,
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
            Tab::Time => {
                ui.add(TimePanel::new(self.sim));
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
        ui.set_min_width(140.0);
        for tab in Tab::ALL {
            if ui.button(tab.title()).clicked() {
                self.to_open.push(tab);
            }
        }
    }
}
