use libc;
use std::ptr;

use crate::{
    frame::LispFrameRef,
    lisp::ExternalPtr,
    remacs_sys::{wr_display_info, XGCValues},
};

use super::{keyboard::KeyboardProcessor, output::OutputRef, term::TerminalRef};

pub struct DisplayInfoInner {
    pub terminal: TerminalRef,
    pub focus_frame: LispFrameRef,

    pub output: OutputRef,

    pub keyboard_processor: KeyboardProcessor,

    pub scratch_cursor_gc: Box<XGCValues>,
}

impl Default for DisplayInfoInner {
    fn default() -> Self {
        DisplayInfoInner {
            terminal: TerminalRef::default(),
            focus_frame: LispFrameRef::new(ptr::null_mut()),
            output: OutputRef::new(ptr::null_mut()),
            keyboard_processor: KeyboardProcessor::new(),
            scratch_cursor_gc: Box::new(XGCValues {
                foreground: 0,
                background: 0,
                font: ptr::null_mut(),
            }),
        }
    }
}

pub type DisplayInfoInnerRef = ExternalPtr<DisplayInfoInner>;

pub type DisplayInfo = wr_display_info;

impl DisplayInfo {
    pub fn new() -> Self {
        let mut df = DisplayInfo::default();

        let inner = Box::new(DisplayInfoInner::default());
        df.inner = Box::into_raw(inner) as *mut libc::c_void;

        df
    }

    pub fn get_inner(&self) -> DisplayInfoInnerRef {
        DisplayInfoInnerRef::new(self.inner as *mut DisplayInfoInner)
    }
}

impl Drop for DisplayInfo {
    fn drop(&mut self) {
        if self.inner != ptr::null_mut() {
            unsafe {
                Box::from_raw(self.inner as *mut DisplayInfoInner);
            }
        }
    }
}

pub type DisplayInfoRef = ExternalPtr<DisplayInfo>;
unsafe impl Sync for DisplayInfoRef {}

impl Default for DisplayInfoRef {
    fn default() -> Self {
        Self::new(ptr::null_mut())
    }
}