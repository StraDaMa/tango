use crate::{audio, battle, font, hooks, input, ipc, tps, video};
use ab_glyph::{Font, ScaleFont};
use parking_lot::Mutex;
use rand::SeedableRng;
use std::sync::Arc;

pub const EXPECTED_FPS: f32 = 60.0;

pub fn run(
    rt: tokio::runtime::Runtime,
    ipc_sender: Arc<Mutex<ipc::Sender>>,
    window_title: String,
    input_mapping: input::Mapping,
    rom_path: std::path::PathBuf,
    save_path: std::path::PathBuf,
    window_scale: u32,
    video_filter: Box<dyn video::Filter>,
    match_init: Option<battle::MatchInit>,
) -> Result<(), anyhow::Error> {
    let handle = rt.handle().clone();

    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let game_controller = sdl.game_controller().unwrap();
    let audio = sdl.audio().unwrap();

    let title_prefix = format!("Tango: {}", window_title);
    let (vbuf_width, vbuf_height) = video_filter.output_size((
        mgba::gba::SCREEN_WIDTH as usize,
        mgba::gba::SCREEN_HEIGHT as usize,
    ));
    let mut vbuf = vec![0u8; (vbuf_width * vbuf_height * 4) as usize];
    let emu_vbuf = Arc::new(Mutex::new(vec![
        0u8;
        (mgba::gba::SCREEN_WIDTH * mgba::gba::SCREEN_HEIGHT * 4)
            as usize
    ]));

    let window = video
        .window(
            &title_prefix,
            std::cmp::max(mgba::gba::SCREEN_WIDTH * window_scale, vbuf_width as u32),
            std::cmp::max(mgba::gba::SCREEN_HEIGHT * window_scale, vbuf_height as u32),
        )
        .opengl()
        .resizable()
        .build()
        .unwrap();

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .unwrap();

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl2::pixels::PixelFormatEnum::ABGR8888,
            vbuf_width as u32,
            vbuf_height as u32,
        )
        .unwrap();

    let audio_cb = audio::LateBinder::<i16>::new();
    let audio_device = audio
        .open_playback(
            None,
            &sdl2::audio::AudioSpecDesired {
                freq: Some(48000),
                channels: Some(audio::NUM_CHANNELS as u8),
                samples: Some(512),
            },
            {
                let audio_cb = audio_cb.clone();
                |_| audio_cb
            },
        )
        .unwrap();
    log::info!("audio spec: {:?}", audio_device.spec());
    audio_device.resume();

    let fps_counter = Arc::new(Mutex::new(tps::Counter::new(30)));
    let emu_tps_counter = Arc::new(Mutex::new(tps::Counter::new(10)));

    let joyflags = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut input_state = sdl2_input_helper::State::new();

    let mut controllers: std::collections::HashMap<u32, sdl2::controller::GameController> =
        std::collections::HashMap::new();
    // Preemptively enumerate controllers.
    for which in 0..game_controller.num_joysticks().unwrap() {
        if !game_controller.is_game_controller(which) {
            continue;
        }
        let controller = game_controller.open(which).unwrap();
        log::info!("controller added: {}", controller.name());
        controllers.insert(which, controller);
    }

    let font =
        ab_glyph::FontRef::try_from_slice(&include_bytes!("fonts/04B_03__.TTF")[..]).unwrap();
    let scale = ab_glyph::PxScale::from(16.0);
    let scaled_font = font.as_scaled(scale);

    let mut event_loop = sdl.event_pump().unwrap();

    {
        let mut core = mgba::core::Core::new_gba("tango")?;
        core.enable_video_buffer();

        let rom = std::fs::read(rom_path)?;
        let rom_vf = mgba::vfile::VFile::open_memory(&rom);
        core.as_mut().load_rom(rom_vf)?;

        log::info!(
            "loaded game: {} rev {}",
            std::str::from_utf8(&core.as_mut().full_rom_name()).unwrap(),
            core.as_mut().rom_revision(),
        );

        let save_vf = if match_init.is_none() {
            mgba::vfile::VFile::open(
                &save_path,
                mgba::vfile::flags::O_CREAT | mgba::vfile::flags::O_RDWR,
            )?
        } else {
            log::info!("in pvp mode, save file will not be written back to disk");
            mgba::vfile::VFile::open_memory(&std::fs::read(save_path)?)
        };

        core.as_mut().load_save(save_vf)?;

        let hooks = hooks::get(core.as_mut()).unwrap();
        hooks.patch(core.as_mut());

        let match_ = std::sync::Arc::new(tokio::sync::Mutex::new(None));
        if let Some(match_init) = match_init.as_ref() {
            let _ = std::fs::create_dir_all(match_init.settings.replays_path.parent().unwrap());
            let mut traps = hooks.common_traps();
            traps.extend(hooks.primary_traps(handle.clone(), joyflags.clone(), match_.clone()));
            core.set_traps(traps);
        }

        let thread = mgba::thread::Thread::new(core);

        let match_ = if let Some(match_init) = match_init {
            let (dc_rx, dc_tx) = match_init.dc.split();

            {
                let match_ = match_.clone();
                handle.block_on(async {
                    let is_offerer = match_init.peer_conn.local_description().unwrap().sdp_type
                        == datachannel_wrapper::SdpType::Offer;
                    let rng_seed = match_init
                        .settings
                        .rng_seed
                        .clone()
                        .try_into()
                        .expect("rng seed");
                    *match_.lock().await = Some(
                        battle::Match::new(
                            rom,
                            hooks,
                            match_init.peer_conn,
                            dc_tx,
                            rand_pcg::Mcg128Xsl64::from_seed(rng_seed),
                            is_offerer,
                            thread.handle(),
                            ipc_sender.clone(),
                            match_init.settings,
                        )
                        .expect("new match"),
                    );
                });
            }

            {
                let match_ = match_.clone();
                handle.spawn(async move {
                    {
                        let match_ = match_.lock().await.clone().unwrap();
                        tokio::select! {
                            Err(e) = match_.run(dc_rx) => {
                                log::info!("match thread ending: {:?}", e);
                            }
                            _ = match_.cancelled() => {
                            }
                        }
                    }
                });
            }

            Some(match_)
        } else {
            None
        };

        thread.start()?;
        thread
            .handle()
            .lock_audio()
            .sync_mut()
            .set_fps_target(EXPECTED_FPS);

        audio_cb.bind(Some(Box::new(audio::MGBAStream::new(
            thread.handle(),
            audio_device.spec().freq,
        ))));

        {
            let joyflags = joyflags.clone();
            let emu_vbuf = emu_vbuf.clone();
            let emu_tps_counter = emu_tps_counter.clone();
            thread.set_frame_callback(move |mut core, video_buffer| {
                let mut emu_vbuf = emu_vbuf.lock();
                emu_vbuf.copy_from_slice(video_buffer);
                for i in (0..emu_vbuf.len()).step_by(4) {
                    emu_vbuf[i + 3] = 0xff;
                }
                core.set_keys(joyflags.load(std::sync::atomic::Ordering::Relaxed));
                let mut emu_tps_counter = emu_tps_counter.lock();
                emu_tps_counter.mark();
            });
        }

        log::info!("running...");
        rt.block_on(async {
            ipc_sender
                .lock()
                .send(ipc::protos::FromCoreMessage {
                    which: Some(ipc::protos::from_core_message::Which::StateEv(
                        ipc::protos::from_core_message::StateEvent {
                            state: ipc::protos::from_core_message::state_event::State::Running
                                .into(),
                        },
                    )),
                })
                .await?;
            anyhow::Result::<()>::Ok(())
        })?;

        let mut show_debug_pressed = false;
        let mut show_debug = false;

        let thread_handle = thread.handle();

        'toplevel: loop {
            // Handle events.
            for event in event_loop.poll_iter() {
                match event {
                    sdl2::event::Event::Quit { .. } => {
                        break 'toplevel;
                    }
                    sdl2::event::Event::ControllerDeviceAdded { which, .. } => {
                        if !game_controller.is_game_controller(which) {
                            continue;
                        }
                        let controller = game_controller.open(which).unwrap();
                        log::info!("controller added: {}", controller.name());
                        controllers.insert(which, controller);
                    }
                    sdl2::event::Event::ControllerDeviceRemoved { which, .. } => {
                        if let Some(controller) = controllers.remove(&which) {
                            log::info!("controller removed: {}", controller.name());
                        }
                    }
                    _ => {}
                }

                if input_state.handle_event(&event) {
                    let last_show_debug_pressed = show_debug_pressed;
                    show_debug_pressed =
                        input_state.is_key_pressed(sdl2::keyboard::Scancode::Grave);
                    if show_debug_pressed && !last_show_debug_pressed {
                        show_debug = !show_debug;
                    }
                    joyflags.store(
                        input_mapping.to_mgba_keys(&input_state),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
            }

            // If we're in single-player mode, allow speedup.
            if match_.is_none() {
                let audio_guard = thread_handle.lock_audio();
                audio_guard.sync_mut().set_fps_target(
                    if input_mapping
                        .speed_up
                        .iter()
                        .any(|c| c.is_active(&input_state))
                    {
                        EXPECTED_FPS * 3.0
                    } else {
                        EXPECTED_FPS
                    },
                );
            }

            if let Some(match_) = &match_ {
                if handle.block_on(async { match_.lock().await.is_none() }) {
                    break 'toplevel;
                }
            }

            // If we've crashed, log the error and panic.
            if thread_handle.has_crashed() {
                // HACK: No better way to lock the core.
                let audio_guard = thread_handle.lock_audio();
                panic!(
                    "mgba thread crashed!\nlr = {:08x}, pc = {:08x}",
                    audio_guard.core().gba().cpu().gpr(14),
                    audio_guard.core().gba().cpu().thumb_pc()
                );
            }

            // Apply stupid video scaling filter that only mint wants 🥴
            video_filter.apply(
                &emu_vbuf.lock(),
                &mut vbuf,
                (
                    mgba::gba::SCREEN_WIDTH as usize,
                    mgba::gba::SCREEN_HEIGHT as usize,
                ),
            );
            texture
                .update(None, &vbuf, vbuf_width as usize * 4)
                .unwrap();
            canvas.clear();

            let viewport = canvas.viewport();
            let scaling_factor = std::cmp::max(
                std::cmp::min(
                    viewport.width() / vbuf_width as u32,
                    viewport.height() / vbuf_height as u32,
                ),
                1,
            );
            let (new_width, new_height) = (
                vbuf_width as u32 * scaling_factor,
                vbuf_height as u32 * scaling_factor,
            );
            canvas
                .copy(
                    &texture,
                    None,
                    sdl2::rect::Rect::new(
                        viewport.x() + (viewport.width() as i32 - new_width as i32) / 2,
                        viewport.y() + (viewport.height() as i32 - new_height as i32) / 2,
                        new_width,
                        new_height,
                    ),
                )
                .unwrap();

            // Update title to show P1/P2 state.
            let mut title = title_prefix.to_string();
            if let Some(match_) = match_.as_ref() {
                rt.block_on(async {
                    if let Some(match_) = &*match_.lock().await {
                        let round_state = match_.lock_round_state().await;
                        if let Some(round) = round_state.round.as_ref() {
                            title = format!("{} [P{}]", title, round.local_player_index() + 1);
                        }
                    }
                });
            }
            canvas.window_mut().set_title(&title).unwrap();

            if show_debug {
                draw_debug(
                    handle.clone(),
                    &match_,
                    &mut canvas,
                    &texture_creator,
                    &scaled_font,
                    &*fps_counter.lock(),
                    &*emu_tps_counter.lock(),
                );
            }

            // Done!
            canvas.present();
            fps_counter.lock().mark();
        }
    }

    log::info!("goodbye");
    Ok(())
}

fn draw_debug(
    handle: tokio::runtime::Handle,
    match_: &Option<std::sync::Arc<tokio::sync::Mutex<Option<std::sync::Arc<battle::Match>>>>>,
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    texture_creator: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    scaled_font: &ab_glyph::PxScaleFont<&ab_glyph::FontRef>,
    fps_counter: &tps::Counter,
    emu_tps_counter: &tps::Counter,
) {
    let mut lines = vec![format!(
        "fps: {:.02}",
        1.0 / fps_counter.mean_duration().as_secs_f32()
    )];

    let tps_adjustment = if let Some(match_) = match_.as_ref() {
        handle.block_on(async {
            if let Some(match_) = &*match_.lock().await {
                lines.push("match active".to_string());
                let round_state = match_.lock_round_state().await;
                if let Some(round) = round_state.round.as_ref() {
                    lines.push(format!(
                        "local player index: {}",
                        round.local_player_index()
                    ));
                    lines.push(format!(
                        "qlen: {} (-{}) vs {} (-{})",
                        round.local_queue_length(),
                        round.local_delay(),
                        round.remote_queue_length(),
                        round.remote_delay(),
                    ));
                    round.tps_adjustment()
                } else {
                    0.0
                }
            } else {
                0.0
            }
        })
    } else {
        0.0
    };

    lines.push(format!(
        "emu tps: {:.02} ({:+.02})",
        1.0 / emu_tps_counter.mean_duration().as_secs_f32(),
        tps_adjustment
    ));

    for (i, line) in lines.iter().enumerate() {
        let mut glyphs = Vec::new();
        font::layout_paragraph(
            scaled_font,
            ab_glyph::point(0.0, 0.0),
            9999.0,
            &line,
            &mut glyphs,
        );

        let height = scaled_font.height().ceil() as i32;
        let width = {
            let min_x = glyphs.first().unwrap().position.x;
            let last_glyph = glyphs.last().unwrap();
            let max_x = last_glyph.position.x + scaled_font.h_advance(last_glyph.id);
            (max_x - min_x).ceil() as i32
        };

        let mut texture = texture_creator
            .create_texture_streaming(
                sdl2::pixels::PixelFormatEnum::ABGR8888,
                width as u32,
                height as u32,
            )
            .unwrap();

        let mut font_buf = vec![0x0u8; (width * height * 4) as usize];
        for glyph in glyphs {
            if let Some(outlined) = scaled_font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, v| {
                    let x = x as i32 + bounds.min.x as i32;
                    let y = y as i32 + bounds.min.y as i32;
                    if x >= width || y >= height || x < 0 || y < 0 {
                        return;
                    }
                    let gray = (v * 0xff as f32) as u8;
                    font_buf[((y * width + x) * 4) as usize + 0] = gray;
                    font_buf[((y * width + x) * 4) as usize + 1] = gray;
                    font_buf[((y * width + x) * 4) as usize + 2] = gray;
                    font_buf[((y * width + x) * 4) as usize + 3] = 0xff;
                });
            }
        }
        texture
            .update(None, &font_buf[..], (width * 4) as usize)
            .unwrap();

        canvas
            .copy(
                &texture,
                None,
                Some(sdl2::rect::Rect::new(
                    1,
                    (1 + i * height as usize) as i32,
                    width as u32,
                    height as u32,
                )),
            )
            .unwrap();
    }
}
