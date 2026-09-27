use lapiz_runtime::{Runtime, plugin::Plugin};

use crate::key::KeyboardState;

pub mod key;
pub mod mouse;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut Runtime) {
        app.add_service::<KeyboardState>();
    }

    fn finish(&self, _app: &mut Runtime) {}
}
