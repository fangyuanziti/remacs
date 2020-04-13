use std::ptr;

use super::{
    display_info::{DisplayInfo, DisplayInfoRef},
    font::WRFontRef,
    glyph::GlyphStringRef,
    output::OutputRef,
};

use crate::{
    frame::LispFrameRef,
    lisp::{ExternalPtr, LispObject},
    remacs_sys::{
        allocate_kboard, create_terminal, current_kboard, frame_parm_handler, glyph_row,
        glyph_string, glyph_type, initial_kboard, output_method, redisplay_interface, terminal,
        xlispstrdup, Fcons, Lisp_Frame, Lisp_Window, Qnil, Qwr, KBOARD,
    },
    remacs_sys::{
        x_clear_end_of_line, x_clear_window_mouse_face, x_fix_overlapping_area,
        x_get_glyph_overhangs, x_insert_glyphs, x_produce_glyphs, x_set_font, x_set_font_backend,
        x_write_glyphs,
    },
};

pub type TerminalRef = ExternalPtr<terminal>;

impl Default for TerminalRef {
    fn default() -> Self {
        Self::new(ptr::null_mut())
    }
}

pub type KboardRef = ExternalPtr<KBOARD>;

impl KboardRef {
    pub fn add_ref(&mut self) {
        (*self).reference_count = (*self).reference_count + 1;
    }
}

type RedisplayInterfaceRef = ExternalPtr<redisplay_interface>;
unsafe impl Sync for RedisplayInterfaceRef {}

fn get_frame_parm_handlers() -> [frame_parm_handler; 45] {
    // Keep this list in the same order as frame_parms in frame.c.
    // Use None for unsupported frame parameters.
    let handlers: [frame_parm_handler; 45] = [
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(x_set_font),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(x_set_font_backend),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];

    handlers
}

lazy_static! {
    static ref REDISPLAY_INTERFACE: RedisplayInterfaceRef = {
        let frame_parm_handlers = Box::new(get_frame_parm_handlers());

        let interface = Box::new(redisplay_interface {
            frame_parm_handlers: (Box::into_raw(frame_parm_handlers)) as *mut Option<_>,
            produce_glyphs: Some(x_produce_glyphs),
            write_glyphs: Some(x_write_glyphs),
            insert_glyphs: Some(x_insert_glyphs),
            clear_end_of_line: Some(x_clear_end_of_line),
            scroll_run_hook: None,
            after_update_window_line_hook: Some(after_update_window_line),
            update_window_begin_hook: Some(update_window_begin),
            update_window_end_hook: Some(update_window_end),
            flush_display: Some(flush_display),
            clear_window_mouse_face: Some(x_clear_window_mouse_face),
            get_glyph_overhangs: Some(x_get_glyph_overhangs),
            fix_overlapping_area: Some(x_fix_overlapping_area),
            draw_fringe_bitmap: None,
            define_fringe_bitmap: None,
            destroy_fringe_bitmap: None,
            compute_glyph_string_overhangs: None,
            draw_glyph_string: Some(draw_glyph_string),
            define_frame_cursor: None,
            clear_frame_area: Some(clear_frame_area),
            draw_window_cursor: None,
            draw_vertical_window_border: None,
            draw_window_divider: None,
            shift_glyphs_for_insert: None, // Never called; see comment in function.
            show_hourglass: None,
            hide_hourglass: None,
        });

        RedisplayInterfaceRef::new(Box::into_raw(interface))
    };
}

#[allow(unused_variables)]
extern "C" fn update_window_begin(w: *mut Lisp_Window) {}

#[allow(unused_variables)]
extern "C" fn update_window_end(
    w: *mut Lisp_Window,
    cursor_no_p: bool,
    mouse_face_overwritten_p: bool,
) {
}

extern "C" fn flush_display(f: *mut Lisp_Frame) {
    let frame: LispFrameRef = f.into();
    let mut output: OutputRef = unsafe { frame.output_data.wr.into() };

    output.flush();
}

#[allow(unused_variables)]
extern "C" fn after_update_window_line(w: *mut Lisp_Window, desired_row: *mut glyph_row) {}

#[allow(unused_variables)]
extern "C" fn draw_glyph_string(s: *mut glyph_string) {
    let s: GlyphStringRef = s.into();

    let type_ = s.first_glyph().type_();

    match type_ {
        glyph_type::CHAR_GLYPH => draw_char_glyph_string(s),
        _ => {}
    }
}

fn draw_char_glyph_string(mut s: GlyphStringRef) {
    let font: WRFontRef = s.font.into();

    let driver = font.font.driver;

    let draw = unsafe { (*driver).draw }.expect("Font driver has no draw function.");

    let x_start = s.x;

    let y_start = s.y + (font.font.ascent + (s.height - font.font.height) / 2);
    unsafe { draw(s.as_mut(), 0, s.nchars, x_start, y_start, true) };
}

#[allow(unused_variables)]
extern "C" fn clear_frame_area(s: *mut Lisp_Frame, x: i32, y: i32, width: i32, height: i32) {}

fn wr_create_terminal(mut dpyinfo: DisplayInfoRef) -> TerminalRef {
    let terminal_ptr = unsafe {
        create_terminal(
            output_method::output_wr,
            REDISPLAY_INTERFACE.clone().as_mut(),
        )
    };

    let mut terminal = TerminalRef::new(terminal_ptr);

    // Link terminal and dpyinfo together
    terminal.display_info.wr = dpyinfo.as_mut();
    dpyinfo.get_inner().terminal = terminal;

    // Other hooks are NULL by default.

    terminal
}

pub fn wr_term_init(display_name: LispObject) -> DisplayInfoRef {
    let dpyinfo = Box::new(DisplayInfo::new());
    let mut dpyinfo_ref = DisplayInfoRef::new(Box::into_raw(dpyinfo));

    let mut terminal = wr_create_terminal(dpyinfo_ref);

    let mut kboard = KboardRef::new(unsafe { allocate_kboard(Qwr) });
    terminal.kboard = kboard.as_mut();

    // Don't let the initial kboard remain current longer than necessary.
    // That would cause problems if a file loaded on startup tries to
    // prompt in the mini-buffer.
    unsafe {
        if current_kboard == initial_kboard {
            current_kboard = terminal.kboard;
        }
    }

    kboard.add_ref();

    dpyinfo_ref.name_list_element = unsafe { Fcons(display_name, Qnil) };

    // https://lists.gnu.org/archive/html/emacs-devel/2015-11/msg00194.html
    dpyinfo_ref.smallest_font_height = 1;
    dpyinfo_ref.smallest_char_width = 1;

    dpyinfo_ref.resx = 1.0;
    dpyinfo_ref.resy = 1.0;

    // Set the name of the terminal.
    terminal.name = unsafe { xlispstrdup(display_name) };

    dpyinfo_ref
}
