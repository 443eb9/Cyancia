#[cfg(target_os = "android")]
#[path = "main.rs"]
mod app;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    app::run(android_app);
}
