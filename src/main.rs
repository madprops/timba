use eframe::{egui, App, Frame};
use image::codecs::gif::GifDecoder;
use image::AnimationDecoder;
use std::env;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc;
use std::thread;

const SOCKET_PATH: &str = "/tmp/timba.sock";

struct TimbaApp {
    texture: Option<egui::TextureHandle>,
    image_path: String,
    error_message: Option<String>,
    original_size: Option<egui::Vec2>,
    image_receiver: mpsc::Receiver<String>,
    gif_frames: Option<Vec<(egui::ColorImage, std::time::Duration)>>,
    current_frame: usize,
    last_frame_time: std::time::Instant,
    is_animated: bool,
    is_maximized: bool,
    history: Vec<String>,
    history_index: usize,
}

impl App for TimbaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        ui.ctx().request_repaint();

        if let Some(real_maximized_state) = ui.input(|i| i.viewport().maximized) {
            self.is_maximized = real_maximized_state;
        }

        let mut load_path = None;

        while let Ok(new_path) = self.image_receiver.try_recv() {
            load_path = Some(new_path);
        }

        ui.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(path) = &i.raw.dropped_files[0].path {
                    load_path = Some(path.to_string_lossy().into_owned());
                }
            }
        });

        if let Some(path) = load_path {
            if !self.history.contains(&path) {
                self.history.push(path.clone());
            }

            if let Some(index) = self.history.iter().position(|p| p == &path) {
                self.history_index = index;
            }

            self.image_path = path;
            self.texture = None;
            self.error_message = None;
            self.original_size = None;
            self.gif_frames = None;
            self.current_frame = 0;
            self.is_animated = false;
            self.load_image(ui.ctx());
        }

        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);

        if scroll_y != 0.0 {
            if !self.history.is_empty() {
                let mut new_index = self.history_index;

                if scroll_y > 0.0 {
                    if self.history_index > 0 {
                        new_index -= 1;
                    }
                } else if scroll_y < 0.0 {
                    if (self.history_index + 1) < self.history.len() {
                        new_index += 1;
                    }
                }

                if new_index != self.history_index {
                    self.history_index = new_index;
                    self.image_path = self.history[self.history_index].clone();
                    self.texture = None;
                    self.error_message = None;
                    self.original_size = None;
                    self.gif_frames = None;
                    self.current_frame = 0;
                    self.is_animated = false;
                    self.load_image(ui.ctx());
                }
            }
        }

        if self.texture.is_none() && !self.image_path.is_empty() && self.error_message.is_none() {
            self.load_image(ui.ctx());
        }

        if self.is_animated {
            if let Some(ref frames) = self.gif_frames {
                let current_time = std::time::Instant::now();

                if self.current_frame < frames.len() {
                    let frame_duration = frames[self.current_frame].1;

                    if current_time.duration_since(self.last_frame_time) >= frame_duration {
                        self.current_frame = (self.current_frame + 1) % frames.len();
                        self.last_frame_time = current_time;
                        self.update_texture(ui.ctx());
                    }
                }
            }
        }

        let response = egui::CentralPanel::default()
            .show(ui, |ui| {
                ui.set_min_size(egui::Vec2::ZERO);

                if let Some(error) = &self.error_message {
                    ui.label(format!("Error: {}", error));
                    return;
                }

                if let Some(texture) = &self.texture {
                    if let Some(original_size) = self.original_size {
                        let available_size = ui.available_size();

                        let scale_x = available_size.x / original_size.x;
                        let scale_y = available_size.y / original_size.y;

                        let scale = scale_x.min(scale_y);

                        let displayed_size =
                            egui::vec2(original_size.x * scale, original_size.y * scale);

                        ui.vertical_centered(|ui| {
                            ui.add_space(((available_size.y - displayed_size.y) / 2.0).max(0.0));

                            let img = egui::Image::new(texture)
                                .fit_to_exact_size(displayed_size)
                                .sense(egui::Sense::click());
                            let img_response = ui.add(img);

                            if img_response.double_clicked() {
                                self.is_maximized = !self.is_maximized;
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
                                    self.is_maximized,
                                ));
                            }

                            if img_response.secondary_clicked() {
                                ui.ctx().copy_text(self.image_path.clone());
                            }
                        });
                    }
                } else {
                    ui.label("Loading image...");
                }
            })
            .response;

        let interact_response = response.interact(egui::Sense::click());

        if interact_response.double_clicked() {
            self.is_maximized = !self.is_maximized;
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(self.is_maximized));
        }

        if interact_response.secondary_clicked() {
            ui.ctx().copy_text(self.image_path.clone());
        }
    }
}

impl TimbaApp {
    fn new(image_path: String, image_receiver: mpsc::Receiver<String>) -> Self {
        Self {
            texture: None,
            image_path: image_path.clone(),
            error_message: None,
            original_size: None,
            image_receiver,
            gif_frames: None,
            current_frame: 0,
            last_frame_time: std::time::Instant::now(),
            is_animated: false,
            is_maximized: false,
            history: vec![image_path],
            history_index: 0,
        }
    }

    fn load_image(&mut self, ctx: &egui::Context) {
        let path = Path::new(&self.image_path);

        if path.extension().and_then(|s| s.to_str()) == Some("gif") {
            self.load_gif(ctx);
        } else {
            self.load_static_image(ctx);
        }
    }

    fn load_static_image(&mut self, ctx: &egui::Context) {
        let path = Path::new(&self.image_path);

        match image::open(path) {
            Ok(img) => {
                let width = img.width() as f32;
                let height = img.height() as f32;
                let size = [img.width() as usize, img.height() as usize];
                let image_buffer = img.to_rgba8();
                let pixels = image_buffer.into_vec();

                self.original_size = Some(egui::vec2(width, height));

                let texture = ctx.load_texture(
                    path.file_name().unwrap().to_string_lossy(),
                    egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
                    egui::TextureOptions::LINEAR,
                );

                self.texture = Some(texture);
                self.is_animated = false;
                self.gif_frames = None;
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to load image: {}", err));
                println!(">>> Failed to load image: {}", err);
            }
        }
    }

    fn load_gif(&mut self, ctx: &egui::Context) {
        let file = match std::fs::File::open(&self.image_path) {
            Ok(file) => file,
            Err(e) => {
                self.error_message = Some(format!("Failed to open file: {}", e));
                return;
            }
        };

        let reader = BufReader::new(file);

        let decoder = match GifDecoder::new(reader) {
            Ok(d) => d,
            Err(e) => {
                self.error_message = Some(format!("Failed to initialize GIF decoder: {}", e));
                return;
            }
        };

        let frames = decoder.into_frames();
        let mut gif_frames = Vec::new();

        for frame_result in frames {
            match frame_result {
                Ok(frame) => {
                    let delay = frame.delay();
                    let duration: std::time::Duration = delay.into();
                    let buffer = frame.into_buffer();
                    let (width, height) = buffer.dimensions();

                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        &buffer.into_raw(),
                    );

                    gif_frames.push((color_image, duration));
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to decode frame: {}", e));
                    return;
                }
            }
        }

        if !gif_frames.is_empty() {
            let (width, height) = (gif_frames[0].0.width(), gif_frames[0].0.height());
            self.original_size = Some(egui::vec2(width as f32, height as f32));
            self.gif_frames = Some(gif_frames);
            self.current_frame = 0;
            self.last_frame_time = std::time::Instant::now();
            self.is_animated = true;
            self.update_texture(ctx);
        }
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        if let Some(ref frames) = self.gif_frames {
            if self.current_frame < frames.len() {
                let color_image = frames[self.current_frame].0.clone();

                if let Some(ref mut texture) = self.texture {
                    texture.set(color_image, egui::TextureOptions::LINEAR);
                } else {
                    self.texture = Some(ctx.load_texture(
                        "gif_frame",
                        color_image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <image_path>", args[0]);
        return Ok(());
    }

    let image_path = args[1].clone();

    let image_path = std::fs::canonicalize(image_path)
        .unwrap_or_else(|_| Path::new(&args[1]).to_path_buf())
        .to_string_lossy()
        .into_owned();

    if !Path::new(&image_path).exists() {
        eprintln!("Error: Image path does not exist: {}", image_path);
        return Ok(());
    }

    if let Ok(mut stream) = UnixStream::connect(SOCKET_PATH) {
        println!(
            "Connected to existing Timba instance, sending path: {}",
            image_path
        );

        if let Err(e) = stream.write_all(image_path.as_bytes()) {
            eprintln!("Failed to send path to existing instance: {}", e);
            return Ok(());
        }

        if let Err(e) = stream.flush() {
            eprintln!("Failed to flush stream: {}", e);
            return Ok(());
        }

        let mut buffer = [0; 3];

        match stream.read(&mut buffer) {
            Ok(bytes) if bytes > 0 => {
                let response = std::str::from_utf8(&buffer[0..bytes]).unwrap_or("???");
                println!("Response from instance: {}", response);
            }
            Ok(_) => {
                println!("No response received from instance");
            }
            Err(e) => {
                println!("Error waiting for acknowledgment: {}", e);
            }
        }

        println!("Image sent to existing Timba instance");
        return Ok(());
    }

    let _ = fs::remove_file(SOCKET_PATH);

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        if let Ok(listener) = UnixListener::bind(SOCKET_PATH) {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    let mut buffer = [0; 4096];

                    match stream.read(&mut buffer) {
                        Ok(bytes_read) if bytes_read > 0 => {
                            let path = String::from_utf8_lossy(&buffer[0..bytes_read]).into_owned();

                            if Path::new(&path).exists() {
                                if let Err(e) = tx.send(path) {
                                    eprintln!("Failed to send image path internally: {}", e);
                                    let _ = stream.write_all(b"ERR");
                                } else {
                                    let _ = stream.write_all(b"OK");
                                }
                            } else {
                                eprintln!("Received path does not exist: {}", path);
                                let _ = stream.write_all(b"ERR");
                            }
                        }
                        Ok(_) => {
                            eprintln!("Received empty path over socket");
                            let _ = stream.write_all(b"ERR");
                        }
                        Err(e) => {
                            eprintln!("Error reading from socket: {}", e);
                            let _ = stream.write_all(b"ERR");
                        }
                    }
                }
            }
        } else {
            eprintln!("Failed to bind to socket {}", SOCKET_PATH);
        }
    });

    let socket_path = SOCKET_PATH.to_string();

    ctrlc::set_handler(move || {
        println!("Cleaning up socket file...");
        let _ = fs::remove_file(&socket_path);
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let app = TimbaApp::new(image_path, rx);

    let viewport = egui::ViewportBuilder::default().with_resizable(true);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native("Timba", options, Box::new(|_cc| Ok(Box::new(app))))?;

    let _ = fs::remove_file(SOCKET_PATH);
    Ok(())
}
