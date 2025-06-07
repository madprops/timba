# Timba Image Viewer

Timba is a simple image viewer application built using Rust. It allows users to load and display images in a graphical window.

## Features

- Load and display images in various formats.
- Simple and intuitive user interface.

## Prerequisites

Before you begin, ensure you have the following installed:

- Rust (latest stable version)
- Cargo (comes with Rust)
- GTK or Iced (for GUI)

## Installation

1. Clone the repository:

   ```bash
   git clone https://github.com/yourusername/timba-image-viewer.git
   cd timba-image-viewer
   ```

2. Install the necessary dependencies. If you are using GTK, you may need to install it via your package manager. For example, on Ubuntu:

   ```bash
   sudo apt-get install libgtk-3-dev
   ```

   If you are using Iced, follow the installation instructions on the Iced GitHub page.

3. Build the project:

   ```bash
   cargo build
   ```

## Usage

To run the image viewer, use the following command:

```bash
cargo run -- path/to/your/image.png
```

Replace `path/to/your/image.png` with the actual path to the image you want to view.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any improvements or bug fixes.

## License

This project is licensed under the MIT License. See the LICENSE file for details.