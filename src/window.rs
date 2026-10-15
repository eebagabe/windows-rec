//! Janela winit desenhada por software (softbuffer). Sem contexto OpenGL/DirectX:
//! o app ocupa uma fracao da memoria e nao disputa a GPU com o jogo.
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{ViewportBuilder, ViewportId};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

pub trait App {
    fn ui(&mut self, ui: &mut egui::Ui);
    /// Chamado ao fechar a janela (ja escondida); pode bloquear para salvar.
    fn on_close(&mut self);
}

type Surface = softbuffer::Surface<OwnedDisplayHandle, Arc<Window>>;

/// Limite de quadros: desenhar por CPU acima disso so gasta processador.
const MIN_FRAME: Duration = Duration::from_millis(33);

struct Running<A> {
    window: Arc<Window>,
    surface: Surface,
    input: egui_winit::State,
    app: A,
}

struct Runner<A, F> {
    ctx: egui::Context,
    viewport: ViewportBuilder,
    factory: Option<F>,
    softbuffer: softbuffer::Context<OwnedDisplayHandle>,
    renderer: EguiSoftwareRender,
    running: Option<Running<A>>,
    next_paint: Option<Instant>,
    last_paint: Instant,
    error: Option<String>,
}

impl<A: App, F: FnOnce(egui::Context, Arc<Window>) -> A> Runner<A, F> {
    fn schedule(&mut self, when: Instant) {
        let when = when.max(self.last_paint + MIN_FRAME);
        self.next_paint = Some(self.next_paint.map_or(when, |t| t.min(when)));
    }

    fn paint(&mut self) -> Result<(), String> {
        let Some(r) = self.running.as_mut() else { return Ok(()) };
        let size = r.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(()); // minimizada
        };
        self.last_paint = Instant::now();
        let raw = r.input.take_egui_input(&r.window);
        let out = self.ctx.run_ui(raw, |ui| r.app.ui(ui));
        r.input.handle_platform_output(&r.window, out.platform_output);
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);

        r.surface.resize(w, h).map_err(|e| e.to_string())?;
        let mut buf = r.surface.buffer_mut().map_err(|e| e.to_string())?;
        buf.fill(0);
        let mut target = BufferMutRef::new(bytemuck::cast_slice_mut(&mut buf), w.get() as usize, h.get() as usize);
        self.renderer.render(&mut target, &prims, &out.textures_delta, out.pixels_per_point);
        buf.present().map_err(|e| e.to_string())?;

        if let Some(vp) = out.viewport_output.get(&ViewportId::ROOT) {
            if vp.repaint_delay < Duration::from_secs(3600) {
                self.schedule(Instant::now() + vp.repaint_delay);
            }
        }
        Ok(())
    }

    fn fail(&mut self, el: &ActiveEventLoop, e: String) {
        self.error = Some(e);
        el.exit();
    }
}

impl<A: App, F: FnOnce(egui::Context, Arc<Window>) -> A> ApplicationHandler<Instant> for Runner<A, F> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        let window = match egui_winit::create_window(&self.ctx, el, &self.viewport) {
            Ok(w) => Arc::new(w),
            Err(e) => return self.fail(el, e.to_string()),
        };
        let surface = match Surface::new(&self.softbuffer, window.clone()) {
            Ok(s) => s,
            Err(e) => return self.fail(el, e.to_string()),
        };
        let input = egui_winit::State::new(
            self.ctx.clone(),
            ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let factory = self.factory.take().expect("janela criada uma vez");
        let app = factory(self.ctx.clone(), window.clone());
        window.request_redraw();
        self.running = Some(Running { window, surface, input, app });
    }

    /// Pedido de repintura do egui (de qualquer thread).
    fn user_event(&mut self, _: &ActiveEventLoop, when: Instant) {
        self.schedule(when);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(r) = self.running.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => {
                r.window.set_visible(false);
                r.app.on_close();
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.paint() {
                    self.fail(el, e);
                }
            }
            other => {
                if r.input.on_window_event(&r.window, &other).repaint {
                    self.schedule(Instant::now());
                }
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        match self.next_paint {
            Some(t) if t <= Instant::now() => {
                self.next_paint = None;
                if let Some(r) = &self.running {
                    r.window.request_redraw();
                }
                el.set_control_flow(ControlFlow::Wait);
            }
            Some(t) => el.set_control_flow(ControlFlow::WaitUntil(t)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }
}

pub fn run<A: App>(
    viewport: ViewportBuilder,
    factory: impl FnOnce(egui::Context, Arc<Window>) -> A,
) -> Result<(), String> {
    let event_loop = EventLoop::<Instant>::with_user_event().build().map_err(|e| e.to_string())?;
    let softbuffer = softbuffer::Context::new(event_loop.owned_display_handle()).map_err(|e| e.to_string())?;
    let ctx = egui::Context::default();
    let proxy = event_loop.create_proxy();
    ctx.set_request_repaint_callback(move |info| {
        let _ = proxy.send_event(Instant::now() + info.delay);
    });
    let mut runner = Runner {
        ctx,
        viewport,
        factory: Some(factory),
        softbuffer,
        renderer: EguiSoftwareRender::new(ColorFieldOrder::Bgra),
        running: None,
        next_paint: None,
        last_paint: Instant::now(),
        error: None,
    };
    event_loop.run_app(&mut runner).map_err(|e| e.to_string())?;
    runner.error.map_or(Ok(()), Err)
}
