mod app;
mod coach;
mod data;
mod figure;
mod music;
mod voice;
mod web;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
