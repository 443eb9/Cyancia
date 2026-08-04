use std::collections::HashMap;
use std::io::{Read, Write};

use cyancia_assets::{asset::Asset, loader::AssetSerializer, store::AssetRegistry};
use cyancia_input::key::KeySequence;
use cyancia_runtime::{
    Services,
    service::{FromServices, Service},
};
use serde::{Deserialize, Serialize};

use crate::{ActionId, keystroke::parse_keystroke};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindingDef {
    pub shortcut: String,
    pub action_name: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_null")]
    pub action_data: serde_json::Value,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_none")]
    pub context: Option<String>,
}

fn is_null(value: &serde_json::Value) -> bool {
    matches!(value, serde_json::Value::Null)
}

fn is_none<T>(value: &Option<T>) -> bool {
    value.is_none()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindingDefManifest {
    pub name: String,
    pub actions: Vec<KeyBindingDef>,
}

impl Asset for KeyBindingDefManifest {
    const TYPE_NAME: &'static str = "key_bindings";
}

#[derive(Default)]
pub struct KeyBindingDefManifestLoader;

#[derive(Debug, thiserror::Error)]
pub enum KeyBindingDefManifestLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    String(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl AssetSerializer for KeyBindingDefManifestLoader {
    type Asset = KeyBindingDefManifest;

    type Error = KeyBindingDefManifestLoaderError;

    fn file_extension() -> &'static str {
        "actions"
    }

    fn read(&self, reader: &mut dyn Read) -> Result<Self::Asset, Self::Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        let manifest: KeyBindingDefManifest = serde_json::from_slice(&buf)?;
        Ok(manifest)
    }

    fn write(
        &self,
        asset: &Self::Asset,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let json = serde_json::to_string(asset)?;
        writer.write_all(json.as_bytes())?;
        Ok(())
    }
}

pub struct KeyBindingDefManifestCollection {
    action_collection: ActionCollection,
}

impl Service for KeyBindingDefManifestCollection {}

impl FromServices for KeyBindingDefManifestCollection {
    fn from_services(services: &Services) -> Self {
        let assets = services.service::<AssetRegistry>();
        let handles = assets.all_handles_of::<KeyBindingDefManifest>().unwrap();
        let manifests = handles
            .into_iter()
            .map(|handle| handle.get().unwrap())
            .collect::<Vec<_>>();

        let mut shortcuts = HashMap::new();
        for manifest in manifests {
            log::info!(
                "Loading {} key bindings from manifest {}",
                manifest.actions.len(),
                manifest.name
            );
            for def in &manifest.actions {
                match parse_keystroke(&def.shortcut) {
                    Ok(shortcut) => {
                        shortcuts.insert(shortcut, ActionId::new(def.action_name.clone().into()));
                    }
                    Err(e) => log::error!(
                        "Error loading keybinding {} triggered by {} with context {:?} and data {}: {}",
                        def.action_name,
                        def.shortcut,
                        def.context,
                        def.action_data,
                        e
                    ),
                }
            }
        }

        Self {
            action_collection: ActionCollection { shortcuts },
        }
    }
}

impl KeyBindingDefManifestCollection {
    pub fn action_collection(&self) -> &ActionCollection {
        &self.action_collection
    }
}

#[derive(Clone)]
pub struct ActionCollection {
    shortcuts: HashMap<KeySequence, ActionId>,
}

impl ActionCollection {
    pub fn get_action_id(&self, shortcut: KeySequence) -> Option<ActionId> {
        self.shortcuts.get(&shortcut).cloned()
    }
}