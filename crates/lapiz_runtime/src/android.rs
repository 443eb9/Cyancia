use lapiz_utils::wrapper;
use winit::platform::android::activity;

use crate::global::{Global, Globals};

wrapper! {
    #[derive(Debug, Clone)]
    pub AndroidApp : activity::AndroidApp
}

impl Global for AndroidApp {}

pub trait AndroidAppExt {
    fn android_app(&self) -> &AndroidApp;
}

impl AndroidAppExt for Globals {
    fn android_app(&self) -> &AndroidApp {
        self.global::<AndroidApp>()
    }
}
