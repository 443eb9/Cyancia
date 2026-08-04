
use std::fmt::Display;

use cyancia_input::key::KeySequence;
use iced_core::keyboard::{Modifiers, key};

pub fn parse_keystroke(source: &str) -> Result<KeySequence, InvalidKeystrokeError> {
    let mut modifiers = Modifiers::empty();
    let mut key_name = None;

    let mut components = source.split('-').peekable();
    while let Some(component) = components.next() {
        if component.eq_ignore_ascii_case("ctrl") {
            modifiers |= Modifiers::CTRL;
            continue;
        }
        if component.eq_ignore_ascii_case("alt") {
            modifiers |= Modifiers::ALT;
            continue;
        }
        if component.eq_ignore_ascii_case("shift") {
            modifiers |= Modifiers::SHIFT;
            continue;
        }
        if component.eq_ignore_ascii_case("secondary") {
            if cfg!(target_os = "macos") {
                modifiers |= Modifiers::LOGO;
            } else {
                modifiers |= Modifiers::CTRL;
            }
            continue;
        }
        if component.eq_ignore_ascii_case("cmd")
            || component.eq_ignore_ascii_case("super")
            || component.eq_ignore_ascii_case("win")
        {
            modifiers |= Modifiers::LOGO;
            continue;
        }
        if component.eq_ignore_ascii_case("fn") {
            continue;
        }

        let mut key_str = component.to_string();

        if let Some(next) = components.peek() {
            if next.is_empty() && source.ends_with('-') {
                key_name = Some(String::from("-"));
                break;
            }
            return Err(InvalidKeystrokeError {
                keystroke: source.to_owned(),
            });
        }

        if component.len() == 1 && component.as_bytes()[0].is_ascii_uppercase() {
            modifiers |= Modifiers::SHIFT;
            key_str.make_ascii_lowercase();
        } else {
            key_str.make_ascii_lowercase();
        }
        key_name = Some(key_str);
    }

    key_name = key_name.or_else(|| {
        if modifiers.contains(Modifiers::SHIFT) {
            modifiers.remove(Modifiers::SHIFT);
            Some("shift".to_string())
        } else if modifiers.contains(Modifiers::CTRL) {
            modifiers.remove(Modifiers::CTRL);
            Some("control".to_string())
        } else if modifiers.contains(Modifiers::ALT) {
            modifiers.remove(Modifiers::ALT);
            Some("alt".to_string())
        } else if modifiers.contains(Modifiers::LOGO) {
            modifiers.remove(Modifiers::LOGO);
            Some("platform".to_string())
        } else {
            None
        }
    });

    let key_name = key_name.ok_or_else(|| InvalidKeystrokeError {
        keystroke: source.to_owned(),
    })?;

    let key = parse_key_name(&key_name).ok_or_else(|| InvalidKeystrokeError {
        keystroke: source.to_owned(),
    })?;

    Ok(KeySequence { key, modifiers })
}

fn parse_key_name(name: &str) -> Option<key::Code> {
    if let Some(byte) = name.as_bytes().first() {
        if name.len() == 1 && byte.is_ascii_lowercase() {
            return Some(match byte {
                b'a' => key::Code::KeyA,
                b'b' => key::Code::KeyB,
                b'c' => key::Code::KeyC,
                b'd' => key::Code::KeyD,
                b'e' => key::Code::KeyE,
                b'f' => key::Code::KeyF,
                b'g' => key::Code::KeyG,
                b'h' => key::Code::KeyH,
                b'i' => key::Code::KeyI,
                b'j' => key::Code::KeyJ,
                b'k' => key::Code::KeyK,
                b'l' => key::Code::KeyL,
                b'm' => key::Code::KeyM,
                b'n' => key::Code::KeyN,
                b'o' => key::Code::KeyO,
                b'p' => key::Code::KeyP,
                b'q' => key::Code::KeyQ,
                b'r' => key::Code::KeyR,
                b's' => key::Code::KeyS,
                b't' => key::Code::KeyT,
                b'u' => key::Code::KeyU,
                b'v' => key::Code::KeyV,
                b'w' => key::Code::KeyW,
                b'x' => key::Code::KeyX,
                b'y' => key::Code::KeyY,
                b'z' => key::Code::KeyZ,
                _ => unreachable!(),
            });
        }
        if name.len() == 1 && byte.is_ascii_digit() {
            return Some(match byte {
                b'0' => key::Code::Digit0,
                b'1' => key::Code::Digit1,
                b'2' => key::Code::Digit2,
                b'3' => key::Code::Digit3,
                b'4' => key::Code::Digit4,
                b'5' => key::Code::Digit5,
                b'6' => key::Code::Digit6,
                b'7' => key::Code::Digit7,
                b'8' => key::Code::Digit8,
                b'9' => key::Code::Digit9,
                _ => unreachable!(),
            });
        }
    }

    Some(match name {
        "," => key::Code::Comma,
        "." => key::Code::Period,
        "/" => key::Code::Slash,
        ";" => key::Code::Semicolon,
        "'" => key::Code::Quote,
        "[" => key::Code::BracketLeft,
        "]" => key::Code::BracketRight,
        "\\" => key::Code::Backslash,
        "-" => key::Code::Minus,
        "=" => key::Code::Equal,
        "`" => key::Code::Backquote,
        "space" => key::Code::Space,
        "enter" => key::Code::Enter,
        "tab" => key::Code::Tab,
        "backspace" => key::Code::Backspace,
        "delete" => key::Code::Delete,
        "escape" => key::Code::Escape,
        "home" => key::Code::Home,
        "end" => key::Code::End,
        "pageup" => key::Code::PageUp,
        "pagedown" => key::Code::PageDown,
        "insert" => key::Code::Insert,
        "capslock" => key::Code::CapsLock,
        "printscreen" => key::Code::PrintScreen,
        "scrolllock" => key::Code::ScrollLock,
        "pause" => key::Code::Pause,
        "numlock" => key::Code::NumLock,
        "up" => key::Code::ArrowUp,
        "down" => key::Code::ArrowDown,
        "left" => key::Code::ArrowLeft,
        "right" => key::Code::ArrowRight,
        "shift" => key::Code::ShiftLeft,
        "control" => key::Code::ControlLeft,
        "alt" => key::Code::AltLeft,
        "platform" => key::Code::SuperLeft,
        "menu" => key::Code::ContextMenu,
        name if name.starts_with('f') && name.len() > 1 => {
            let index: usize = name[1..].parse().ok()?;
            if !(1..=24).contains(&index) {
                return None;
            }
            match index {
                1 => key::Code::F1,
                2 => key::Code::F2,
                3 => key::Code::F3,
                4 => key::Code::F4,
                5 => key::Code::F5,
                6 => key::Code::F6,
                7 => key::Code::F7,
                8 => key::Code::F8,
                9 => key::Code::F9,
                10 => key::Code::F10,
                11 => key::Code::F11,
                12 => key::Code::F12,
                13 => key::Code::F13,
                14 => key::Code::F14,
                15 => key::Code::F15,
                16 => key::Code::F16,
                17 => key::Code::F17,
                18 => key::Code::F18,
                19 => key::Code::F19,
                20 => key::Code::F20,
                21 => key::Code::F21,
                22 => key::Code::F22,
                23 => key::Code::F23,
                24 => key::Code::F24,
                _ => unreachable!(),
            }
        }
        _ => return None,
    })
}

#[derive(Debug)]
pub struct InvalidKeystrokeError {
    pub keystroke: String,
}

impl Display for InvalidKeystrokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid keystroke \"{}\". Expected a sequence of modifiers \
             (`ctrl`, `alt`, `shift`, `fn`, `cmd`, `super`, or `win`) \
             followed by a key, separated by `-`.",
            self.keystroke
        )
    }
}

impl std::error::Error for InvalidKeystrokeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_and_key() {
        let seq = parse_keystroke("ctrl-shift-up").unwrap();
        assert_eq!(seq.key, key::Code::ArrowUp);
        assert!(seq.modifiers.contains(Modifiers::CTRL));
        assert!(seq.modifiers.contains(Modifiers::SHIFT));
        assert!(!seq.modifiers.contains(Modifiers::ALT));
    }

    #[test]
    fn parses_function_key() {
        let seq = parse_keystroke("f5").unwrap();
        assert_eq!(seq.key, key::Code::F5);
        assert_eq!(seq.modifiers, Modifiers::empty());
    }

    #[test]
    fn parses_uppercase_key_as_shift() {
        let seq = parse_keystroke("ctrl-O").unwrap();
        assert_eq!(seq.key, key::Code::KeyO);
        assert!(seq.modifiers.contains(Modifiers::CTRL));
        assert!(seq.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn parses_modifier_as_key() {
        let seq = parse_keystroke("ctrl").unwrap();
        assert_eq!(seq.key, key::Code::ControlLeft);
        assert_eq!(seq.modifiers, Modifiers::empty());
    }

    #[test]
    fn rejects_invalid_keystroke() {
        assert!(parse_keystroke("ctrl-o-x").is_err());
        assert!(parse_keystroke("").is_err());
        assert!(parse_keystroke("notakey").is_err());
    }
}
