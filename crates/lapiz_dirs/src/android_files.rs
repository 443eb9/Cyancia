use std::{path::{Path, PathBuf}, sync::Mutex};

use futures::channel::oneshot;
use jni::{JNIEnv, JValue, JavaVM, jni_sig, jni_str, objects::{JObject, JString}};

static PICKER: Mutex<Option<oneshot::Sender<Option<PathBuf>>>> = Mutex::new(None);

fn with_activity<T>(f: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<T>) -> jni::errors::Result<T> {
    let context = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    vm.attach_current_thread(|env| {
        let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
        f(env, &activity)
    })
}

async fn pick(name: Option<&str>) -> Option<PathBuf> {
    let (sender, receiver) = oneshot::channel();
    {
        let mut pending = PICKER.lock().unwrap();
        if pending.is_some() {
            return None;
        }
        *pending = Some(sender);
    }
    let result = with_activity(|env, activity| {
        if let Some(name) = name {
            let name = env.new_string(name)?;
            env.call_method(activity, jni_str!("pickSave"), jni_sig!("(Ljava/lang/String;)V"), &[JValue::Object(&name)])?;
        } else {
            env.call_method(activity, jni_str!("pickOpen"), jni_sig!("()V"), &[])?;
        }
        Ok(())
    });
    if let Err(error) = result {
        PICKER.lock().unwrap().take();
        eprintln!("Cannot launch Android file picker: {error}");
        return None;
    }
    receiver.await.ok().flatten()
}

pub async fn pick_open() -> Option<PathBuf> {
    pick(None).await
}

pub async fn pick_save(name: &str) -> Option<PathBuf> {
    pick(Some(name)).await
}

fn transfer(method: &str, path: &Path) -> bool {
    let Some(path) = path.to_str() else { return false };
    match with_activity(|env, activity| {
        let path = env.new_string(path)?;
        let method = jni::strings::JNIString::new(method);
        env.call_method(activity, &method, jni_sig!("(Ljava/lang/String;)Z"), &[JValue::Object(&path)])?.z()
    }) {
        Ok(ok) => ok,
        Err(error) => {
            eprintln!("Android file transfer failed: {error}");
            false
        }
    }
}

pub fn refresh(path: &Path) -> bool {
    transfer("refresh", path)
}

pub fn commit(path: &Path) -> bool {
    transfer("commit", path)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_lapiz_dev_LapizActivity_filePicked(
    mut env: JNIEnv,
    _activity: JObject,
    path: JString,
) {
    let path = env.with_env(|env| -> jni::errors::Result<_> {
        if path.is_null() { Ok(None) } else { Ok(Some(PathBuf::from(path.try_to_string(env)?))) }
    }).into_outcome();
    if let Some(sender) = PICKER.lock().unwrap().take() {
        let _ = sender.send(match path {
            jni::Outcome::Ok(path) => path,
            _ => None,
        });
    }
}
