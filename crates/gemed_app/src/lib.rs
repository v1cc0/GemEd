mod app;

pub use app::App;

pub fn launch() {
    dioxus::launch(App);
}
