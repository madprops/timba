use eframe::{egui, App, Frame};
#[allow(unused_imports)]
use image::io::Reader as ImageReader;
use image::GenericImageView;
use std::env;
use std::path::Path;
use std::os::unix::net::{UnixStream, UnixListener};
use std::io::{Read, Write};
use std::thread;
use std::sync::mpsc;
use std::fs;

const SOCKET_PATH: &str = "/tmp/timba.sock";

struct TimbaApp {
    texture: Option<egui::TextureHandle>,
    image_path: String,
    error_message: Option<String>,
    original_size: Option<egui::Vec2>,
    image_receiver: mpsc::Receiver<String>,
}

impl App for TimbaApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        // Check for new image path requests
        if let Ok(new_path) = self.image_receiver.try_recv() {
            println!(">>> Received new image path in UI thread: {}", new_path);
            println!(">>> Previous path was: {}", self.image_path);
            self.image_path = new_path;
            self.texture = None;
            self.error_message = None;
            self.original_size = None;

            // Reset the resize flag - but don't resize window for subsequent images
            unsafe {
                RESIZED.store(true, std::sync::atomic::Ordering::Relaxed); // Already resized, don't resize again
            }

            // Load the image immediately instead of waiting for next frame
            self.load_image(ctx);

            // Request multiple repaints to ensure UI updates quickly
            ctx.request_repaint();
            println!(">>> Requested UI repaint");
        }

        // Now only load the image on first update since we're loading immediately above
        if self.texture.is_none() && !self.image_path.is_empty() {
            println!(">>> Loading image: {}", self.image_path);
            self.load_image(ctx);
            ctx.request_repaint(); // Request another repaint after loading
        }

        // Rest of the update function remains the same
        egui::CentralPanel::default().show(ctx, |ui| {
            // Show error message if any
            if let Some(error) = &self.error_message {
                ui.label(format!("Error: {}", error));
                return;
            }

            // Show the image with proper scaling
            if let Some(texture) = &self.texture {
                if let Some(original_size) = self.original_size {
                    // Get available space in the panel
                    let available_size = ui.available_size();

                    // Calculate scale factor to fit the image in the available space
                    let scale_x = available_size.x / original_size.x;
                    let scale_y = available_size.y / original_size.y;
                    let scale = scale_x.min(scale_y).min(1.0); // Don't scale above 100%

                    // Calculate displayed size
                    let displayed_size = egui::vec2(
                        original_size.x * scale,
                        original_size.y * scale
                    );

                    // Center the image
                    let padding_x = (available_size.x - displayed_size.x) / 2.0;
                    let padding_y = (available_size.y - displayed_size.y) / 2.0;

                    ui.allocate_space(egui::vec2(available_size.x, padding_y));
                    ui.horizontal(|ui| {
                        ui.add_space(padding_x);
                        ui.add(egui::Image::new(texture, displayed_size));
                    });
                }

                // Initial resize to fit image with some padding - only for first image
                if self.original_size.is_some() {
                    unsafe {
                        if !RESIZED.load(std::sync::atomic::Ordering::Relaxed) {
                            let original_size = self.original_size.unwrap();
                            let window_size = egui::vec2(
                                original_size.x + 40.0,
                                original_size.y + 60.0
                            );
                            frame.set_window_size(window_size);
                            RESIZED.store(true, std::sync::atomic::Ordering::Relaxed);
                            println!(">>> Window resized for first image");
                        }
                    }
                }
            } else {
                ui.label("Loading image...");
            }
        });
    }
}

// Make RESIZED static variable accessible throughout the code
static mut RESIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl TimbaApp {
    fn new(image_path: String, image_receiver: mpsc::Receiver<String>) -> Self {
        Self {
            texture: None,
            image_path,
            error_message: None,
            original_size: None,
            image_receiver,
        }
    }

    // The load_image function
    fn load_image(&mut self, ctx: &egui::Context) {
        let path = Path::new(&self.image_path);

        // Try to load the image
        match image::open(path) {
            Ok(img) => {
                let width = img.width() as f32;
                let height = img.height() as f32;
                let size = [img.width() as _, img.height() as _];
                let image_buffer = img.to_rgba8();
                let pixels = image_buffer.into_vec();

                // Store original dimensions
                self.original_size = Some(egui::vec2(width, height));

                // Create texture
                let texture = ctx.load_texture(
                    path.file_name().unwrap().to_string_lossy(),
                    egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
                    egui::TextureFilter::Linear,
                );

                self.texture = Some(texture);
                println!(">>> Image loaded successfully: {}x{}", width, height);
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to load image: {}", err));
                println!(">>> Failed to load image: {}", err);
            }
        }
    }
}

// Load the embedded icon
fn load_embedded_icon() -> Option<eframe::IconData> {
    // Include the icon directly in the binary
    let icon_bytes = include_bytes!("../img/icon.png");

    match image::load_from_memory(icon_bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = img.dimensions();

            println!("Successfully loaded embedded icon: {}x{} pixels", width, height);

            Some(eframe::IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            })
        }
        Err(err) => {
            eprintln!("Failed to load embedded icon: {}", err);
            None
        }
    }
}

fn get_image_dimensions(path: &str) -> Option<(u32, u32)> {
    ImageReader::open(path).ok()?
        .into_dimensions().ok()
}

fn main() {
    // Get command line arguments
    let args: Vec<String> = env::args().collect();

    // Check if an image path was provided
    if args.len() < 2 {
        eprintln!("Usage: {} <image_path>", args[0]);
        return;
    }

    let image_path = args[1].clone();

    // Normalize and validate the path
    let image_path = std::fs::canonicalize(image_path).unwrap_or_else(|_| Path::new(&args[1]).to_path_buf()).to_string_lossy().into_owned();

    if !Path::new(&image_path).exists() {
        eprintln!("Error: Image path does not exist: {}", image_path);
        return;
    }

    // Try to connect to existing instance
    if let Ok(mut stream) = UnixStream::connect(SOCKET_PATH) {
        // Send the image path to the existing instance
        println!("Connected to existing Timba instance, sending path: {}", image_path);

        // Send the full path to the running instance
        if let Err(e) = stream.write_all(image_path.as_bytes()) {
            eprintln!("Failed to send path to existing instance: {}", e);
            return;
        }

        // Ensure the stream is flushed so all data is sent
        if let Err(e) = stream.flush() {
            eprintln!("Failed to flush stream: {}", e);
            return;
        }

        // Wait for acknowledgment
        let mut buffer = [0; 3];
        match stream.read(&mut buffer) {
            Ok(bytes) if bytes > 0 => {
                let response = std::str::from_utf8(&buffer[0..bytes]).unwrap_or("???");
                println!("Response from instance: {}", response);
            },
            Ok(_) => println!("No response received from instance"),
            Err(e) => println!("Error waiting for acknowledgment: {}", e),
        }
        println!("Image sent to existing Timba instance");
        return; // Exit this instance
    }

    // If we reach here, no existing instance, so become the singleton
    // Remove any stale socket file
    let _ = fs::remove_file(SOCKET_PATH);

    // Create communication channel for the socket listener thread
    let (tx, rx) = mpsc::channel();

    // Start listening for new connections
    thread::spawn(move || {
        if let Ok(listener) = UnixListener::bind(SOCKET_PATH) {
            println!("Listening on socket for new image paths");
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    let mut buffer = [0; 4096];  // Create a fixed-size buffer for the path
                    match stream.read(&mut buffer) {
                        Ok(bytes_read) if bytes_read > 0 => {
                            // Convert the bytes to a string, ignoring any non-UTF8 characters
                            let path = String::from_utf8_lossy(&buffer[0..bytes_read]).into_owned();
                            println!("Socket received path: {}", path);
                            // Make sure we're getting a valid path
                            if Path::new(&path).exists() {
                                println!("Path exists, sending to main thread");
                                // Send path to main thread and acknowledge receipt
                                if let Err(e) = tx.send(path) {
                                    eprintln!("Failed to send image path internally: {}", e);
                                    let _ = stream.write_all(b"ERR");
                                } else {
                                    // Send acknowledgment back to client
                                    let _ = stream.write_all(b"OK");
                                }
                            } else {
                                eprintln!("Received path does not exist: {}", path);
                                let _ = stream.write_all(b"ERR");
                            }
                        },
                        Ok(_) => {
                            eprintln!("Received empty path over socket");
                            let _ = stream.write_all(b"ERR");
                        },
                        Err(e) => {
                            eprintln!("Error reading from socket: {}", e);
                            let _ = stream.write_all(b"ERR");
                        },
                    }
                }
            }
        } else {
            eprintln!("Failed to bind to socket {}", SOCKET_PATH);
        }
    });

    // Set up cleanup of socket file when program exits
    let socket_path = SOCKET_PATH.to_string();
    ctrlc::set_handler(move || {
        println!("Cleaning up socket file...");
        let _ = fs::remove_file(&socket_path);
        std::process::exit(0);
    }).expect("Error setting Ctrl-C handler");

    // Get image dimensions for optimal window sizing
    let initial_size = if let Some((width, height)) = get_image_dimensions(&image_path) {
        egui::vec2(
            (width as f32 + 40.0).min(1200.0), // Cap max size
            (height as f32 + 60.0).min(800.0)
        )
    } else {
        egui::vec2(800.0, 600.0)
    };

    let app = TimbaApp::new(image_path, rx);
    // Use embedded icon instead of loading from file system
    let icon_data = load_embedded_icon();

    let options = eframe::NativeOptions {
        initial_window_size: Some(initial_size),
        resizable: true,
        icon_data,
        ..Default::default()
    };

    eframe::run_native(
        "Timba",
        options,
        Box::new(|_cc| Box::new(app)),
    );

    // Clean up socket when exiting normally
    let _ = fs::remove_file(SOCKET_PATH);
}