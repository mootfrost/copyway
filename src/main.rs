use std::{fs::File, os::unix::io::AsFd};

use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_registry, wl_seat, wl_shm, wl_shm_pool,
        wl_surface,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};

use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1,
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity}
};

fn main() {
    let conn = Connection::connect_to_env().unwrap();

    let mut event_queue = conn.new_event_queue();
    let qhandle = event_queue.handle();

    let display = conn.display();
    display.get_registry(&qhandle, ());

    let mut state = State {
        running: true,
        base_surface: None,
        buffer: None,
        layer_shell: None,
        layer_surface: None,
        configured: false,
    };
    
    while state.running {
        event_queue.blocking_dispatch(&mut state).unwrap();
    }
}

struct State {
    running: bool,
    base_surface: Option<wl_surface::WlSurface>,
    buffer: Option<wl_buffer::WlBuffer>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, .. } = event {
            match &interface[..] {
                "wl_compositor" => {
                    let compositor =
                        registry.bind::<wl_compositor::WlCompositor, _, _>(name, 1, qh, ());
                    let surface = compositor.create_surface(qh, ());
                    state.base_surface = Some(surface);

                    if state.layer_shell.is_some() {
                        state.init_layer_surface(qh);
                    }
                }
                "wl_shm" => {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(name, 1, qh, ());

                    let (width, height) = (800, 600); 
                    let stride = width * 4;
                    let size = stride * height;

                    let mut file = tempfile::tempfile().unwrap();
                    draw(&mut file, (width, height));
                    let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
                    let buffer = pool.create_buffer(
                        0,
                        width as i32,
                        height as i32,
                        stride as i32,
                        wl_shm::Format::Argb8888,
                        qh,
                        (),
                    );
                    state.buffer = Some(buffer);

                    if state.configured {
                        state.attach_buffer();
                    }
                }
                "zwlr_layer_shell_v1" => {
                    let layer_shell = registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                        name,
                        4, 
                        qh,
                        ()
                    );
                    state.layer_shell = Some(layer_shell);

                    if state.base_surface.is_some() {
                        state.init_layer_surface(qh);
                    }
                }
                "wl_seat" => {
                    registry.bind::<wl_seat::WlSeat, _, _>(name, 1, qh, ());
                }
                _ => {}
            }
        }
    }
}

impl State {
    fn init_layer_surface(&mut self, qh: &QueueHandle<Self>) {
        let layer_shell = self.layer_shell.as_ref().unwrap();
        let surface = self.base_surface.as_ref().unwrap();

        let layer_surface = layer_shell.get_layer_surface(
            surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "copyway".into(),
            qh,
            ()
        );

        layer_surface.set_size(0, 300);
        layer_surface.set_anchor(Anchor::Bottom | Anchor::Left | Anchor::Right );
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer_surface.set_margin(0 , 0, 100, 0);

        self.layer_surface = Some(layer_surface);
        surface.commit();
    }

    fn attach_buffer(&mut self) {
        if let (Some(surface), Some(buffer)) = (self.base_surface.as_ref(), self.buffer.as_ref()) {
            surface.attach(Some(buffer), 0, 0);
            surface.commit();
        }
    }
}

fn draw(tmp: &mut File, (buf_x, buf_y): (u32, u32)) {
    use std::{cmp::min, io::Write};
    let mut buf = std::io::BufWriter::new(tmp);
    for y in 0..buf_y {
        for x in 0..buf_x {
            let a = 0xFF;
            let r = min(((buf_x - x) * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
            let g = min((x * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
            let b = min(((buf_x - x) * 0xFF) / buf_x, (y * 0xFF) / buf_y);
            buf.write_all(&[b as u8, g as u8, r as u8, a as u8]).unwrap();
        }
    }
    buf.flush().unwrap();
}

delegate_noop!(State: ignore wl_compositor::WlCompositor);
delegate_noop!(State: ignore wl_surface::WlSurface);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, width, height } = event {
            layer_surface.ack_configure(serial);
            state.configured = true;
            state.attach_buffer();
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(capabilities) } = event {
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key { key, .. } = event {
            if key == 1 {
                state.running = false;
            }
        }
    }
}