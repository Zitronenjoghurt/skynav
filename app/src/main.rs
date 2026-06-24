#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([720.0, 420.0])
            .with_drag_and_drop(true)
            .with_title("skynav")
            .with_app_id("io.github.zitronenjoghurt.skynav"),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "skynav",
        native_options,
        Box::new(|cc| Ok(Box::new(skynav_app::SkyNav::new(cc)))),
    )
    .expect("Failed to run egui application.");
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    console_error_panic_hook::set_once();
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("app_canvas")
            .expect("Failed to find app_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("app_canvas was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(skynav_app::SkyNav::new(cc)))),
            )
            .await;

        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
