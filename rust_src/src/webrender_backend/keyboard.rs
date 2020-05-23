use glutin::event::{ModifiersState, VirtualKeyCode};

use crate::lisp::LispObject;

use crate::remacs_sys::{
    _bindgen_ty_36::{ctrl_modifier, meta_modifier, shift_modifier, super_modifier},
    event_kind, input_event, scroll_bar_part, Qnil,
};

pub struct KeyboardProcessor {
    pub modifiers: ModifiersState,
    pub suppress_chars: bool,
}

impl KeyboardProcessor {
    pub fn new() -> KeyboardProcessor {
        KeyboardProcessor {
            modifiers: ModifiersState::empty(),
            suppress_chars: false,
        }
    }

    pub fn receive_char(&self, c: char, top_frame: LispObject) -> Option<input_event> {
        if self.suppress_chars {
            return None;
        }

        let iev = input_event {
            _bitfield_1: input_event::new_bitfield_1(
                event_kind::ASCII_KEYSTROKE_EVENT,
                scroll_bar_part::scroll_bar_nowhere,
            ),
            code: Self::remove_control(c) as u32,
            modifiers: Self::to_emacs_modifiers(self.modifiers),
            x: 0.into(),
            y: 0.into(),
            timestamp: 0,
            frame_or_window: top_frame,
            arg: Qnil,
        };

        Some(iev)
    }

    pub fn key_pressed(
        &mut self,
        key_code: VirtualKeyCode,
        top_frame: LispObject,
    ) -> Option<input_event> {
        if winit_keycode_emacs_key_name(key_code).is_null() {
            return None;
        }

        self.suppress_chars = true;

        let iev = crate::remacs_sys::input_event {
            _bitfield_1: crate::remacs_sys::input_event::new_bitfield_1(
                crate::remacs_sys::event_kind::NON_ASCII_KEYSTROKE_EVENT,
                crate::remacs_sys::scroll_bar_part::scroll_bar_nowhere,
            ),
            code: key_code as u32,
            modifiers: Self::to_emacs_modifiers(self.modifiers),
            x: 0.into(),
            y: 0.into(),
            timestamp: 0,
            frame_or_window: top_frame,
            arg: Qnil,
        };

        Some(iev)
    }

    pub fn key_released(&mut self) {
        self.suppress_chars = false;
    }

    pub fn change_modifiers(&mut self, modifiers: ModifiersState) {
        self.modifiers = modifiers;
    }

    fn remove_control(c: char) -> char {
        let mut c = c as u8;

        if c < 32 {
            let mut new_c = c + 64;

            if new_c >= 65 && new_c <= 90 {
                new_c += 32;
            }

            c = new_c;
        }

        c as char
    }

    fn to_emacs_modifiers(modifiers: ModifiersState) -> u32 {
        let mut emacs_modifiers: u32 = 0;

        if modifiers.alt() {
            emacs_modifiers |= meta_modifier;
        }
        if modifiers.shift() {
            emacs_modifiers |= shift_modifier;
        }
        if modifiers.ctrl() {
            emacs_modifiers |= ctrl_modifier;
        }
        if modifiers.logo() {
            emacs_modifiers |= super_modifier;
        }

        emacs_modifiers
    }
}

// macro for building key_name c string
macro_rules! kn {
    ($e:expr) => {
        concat!($e, '\0').as_ptr() as *const libc::c_char
    };
}

pub fn winit_keycode_emacs_key_name(keycode: VirtualKeyCode) -> *const libc::c_char {
    match keycode {
        VirtualKeyCode::Escape => kn!("escape"),
        VirtualKeyCode::Back => kn!("backspace"),
        VirtualKeyCode::Return => kn!("return"),
        _ => std::ptr::null(), // null pointer
    }
}
