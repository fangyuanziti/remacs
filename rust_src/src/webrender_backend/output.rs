use gleam::gl;
use glutin::{self, EventsLoop, GlContext, GlWindow};
use webrender::{self, api::units::*, api::*, Renderer};

use crate::{lisp::ExternalPtr, remacs_sys::wr_output};

use super::display_info::DisplayInfoRef;
use super::font::FontRef;

pub struct Output {
    pub output: wr_output,
    pub display_info: DisplayInfoRef,
    pub font: FontRef,
    pub fontset: i32,

    pub window: GlWindow,
    pub renderer: Renderer,
    pub render_api: RenderApi,
    pub events_loop: EventsLoop,
}

impl Output {
    pub fn new() -> Self {
        let (api, renderer, window, events_loop) = Self::create_webrender_window();

        Self {
            output: wr_output::default(),
            display_info: DisplayInfoRef::default(),
            font: FontRef::default(),
            fontset: 0,
            window,
            renderer,
            render_api: api,
            events_loop,
        }
    }

    fn create_webrender_window() -> (RenderApi, Renderer, GlWindow, EventsLoop) {
        let events_loop = glutin::EventsLoop::new();
        let window = glutin::WindowBuilder::new().with_visibility(false);
        let context = glutin::ContextBuilder::new();
        let gl_window = glutin::GlWindow::new(window, context, &events_loop).unwrap();

        unsafe { gl_window.make_current().unwrap() };

        let gl = match gl_window.get_api() {
            glutin::Api::OpenGl => unsafe {
                gl::GlFns::load_with(|symbol| gl_window.get_proc_address(symbol) as *const _)
            },
            glutin::Api::OpenGlEs => unsafe {
                gl::GlesFns::load_with(|symbol| gl_window.get_proc_address(symbol) as *const _)
            },
            glutin::Api::WebGl => unimplemented!(),
        };

        let device_pixel_ratio = gl_window.get_hidpi_factor() as f32;

        let device_size = {
            let size = gl_window
                .get_inner_size()
                .unwrap()
                .to_physical(device_pixel_ratio as f64);
            DeviceIntSize::new(size.width as i32, size.height as i32)
        };

        let webrender_opts = webrender::RendererOptions {
            device_pixel_ratio,
            clear_color: Some(ColorF::new(0.3, 0.0, 0.0, 1.0)),
            debug_flags: webrender::DebugFlags::ECHO_DRIVER_MESSAGES,
            ..webrender::RendererOptions::default()
        };

        let notifier = Box::new(Notifier::new(events_loop.create_proxy()));
        let (renderer, sender) =
            webrender::Renderer::new(gl.clone(), notifier, webrender_opts, None, device_size)
                .unwrap();

        let api = sender.create_api();

        (api, renderer, gl_window, events_loop)
    }

    pub fn show_window(&self) {
        self.window.show();
    }

    pub fn hide_window(&self) {
        self.window.hide();
    }
}

pub type OutputRef = ExternalPtr<Output>;

impl From<*mut wr_output> for OutputRef {
    fn from(ptr: *mut wr_output) -> OutputRef {
        OutputRef::new(ptr as *mut Output)
    }
}

struct Notifier {
    events_proxy: glutin::EventsLoopProxy,
}

impl Notifier {
    fn new(events_proxy: glutin::EventsLoopProxy) -> Notifier {
        Notifier { events_proxy }
    }
}

impl RenderNotifier for Notifier {
    fn clone(&self) -> Box<dyn RenderNotifier> {
        Box::new(Notifier {
            events_proxy: self.events_proxy.clone(),
        })
    }

    fn wake_up(&self) {
        let _ = self.events_proxy.wakeup();
    }

    fn new_frame_ready(
        &self,
        _: DocumentId,
        _scrolled: bool,
        _composite_needed: bool,
        _render_time: Option<u64>,
    ) {
        self.wake_up();
    }
}
