use log::{error, warn};
use scape_shared::{CallbackRef, ConfigMessage, Mods};
use xkbcommon::xkb::Keysym;

use crate::InputState;

impl InputState {
    pub(crate) fn keymap(&mut self, key_name: String, mut mods: Mods, callback: CallbackRef) {
        let key = match key_name.to_lowercase().as_str() {
            "left" => Keysym::Left,
            "right" => Keysym::Right,
            "up" => Keysym::Up,
            "down" => Keysym::Down,
            "f1" => Keysym::F1,
            "f2" => Keysym::F2,
            "f3" => Keysym::F3,
            "f4" => Keysym::F4,
            "f5" => Keysym::F5,
            "f6" => Keysym::F6,
            "f7" => Keysym::F7,
            "f8" => Keysym::F8,
            "f9" => Keysym::F9,
            "f10" => Keysym::F10,
            "f11" => Keysym::F11,
            "f12" => Keysym::F12,
            "f13" => Keysym::F13,
            "f14" => Keysym::F14,
            "f15" => Keysym::F15,
            "f16" => Keysym::F16,
            "f17" => Keysym::F17,
            "f18" => Keysym::F18,
            "f19" => Keysym::F19,
            "f20" => Keysym::F20,
            "f21" => Keysym::F21,
            "f22" => Keysym::F22,
            "f23" => Keysym::F23,
            "f24" => Keysym::F24,
            "f25" => Keysym::F25,
            "f26" => Keysym::F26,
            "f27" => Keysym::F27,
            "f28" => Keysym::F28,
            "f29" => Keysym::F29,
            "f30" => Keysym::F30,
            "f31" => Keysym::F31,
            "f32" => Keysym::F32,
            "f33" => Keysym::F33,
            "f34" => Keysym::F34,
            "f35" => Keysym::F35,
            "xf86_audioplay" => Keysym::XF86_AudioPlay,
            "xf86_audionext" => Keysym::XF86_AudioNext,
            "xf86_audioprev" => Keysym::XF86_AudioPrev,
            "xf86_audiomute" => Keysym::XF86_AudioMute,
            "xf86_audioraisevolume" => Keysym::XF86_AudioRaiseVolume,
            "xf86_audiolowervolume" => Keysym::XF86_AudioLowerVolume,
            "backspace" => Keysym::BackSpace,
            key => {
                let Some(mut c) = key.chars().next() else {
                    warn!("Key for keymap is empty");
                    return;
                };
                if c.is_uppercase() {
                    mods.shift = true;
                }
                if mods.shift {
                    let Some(uppercase_c) = c.to_uppercase().next() else {
                        error!("Changing the case of {c} to uppercase failed");
                        return;
                    };
                    c = uppercase_c;
                }
                Keysym::from_char(c)
            }
        };

        let previous_keymap = self.keymaps.entry(mods).or_default().insert(key, callback);
        if let Some(previous_keymap) = previous_keymap {
            self.comms
                .config(ConfigMessage::ForgetCallback(previous_keymap));
        }
    }
}
