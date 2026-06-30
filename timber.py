import os
import time
import random
import subprocess
import socket

BASE_DIR = "/mnt/struct_1/pics/"
INTERVAL_SECONDS = 60
VALID_EXTENSIONS = {".jpg", ".jpeg", ".png", ".gif"}
SOCKET_PATH = "/tmp/timba.sock"
BINARY_PATH = "target/release/timba"


def get_all_images(base_dir):
    images = []

    for root, dirs_, files in os.walk(base_dir):
        for file in files:
            ext = os.path.splitext(file)[1].lower()

            if ext in VALID_EXTENSIONS:
                images.append(os.path.join(root, file))

    return images


def send_to_socket(image_path):
    """Sends the new path to the running Timba instance."""
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.connect(SOCKET_PATH)
            client.sendall(image_path.encode("utf-8"))
            response = client.recv(3)
            return response == b"OK"
    except (ConnectionRefusedError, FileNotFoundError):
        return False


def main():
    images = get_all_images(BASE_DIR)

    if not images:
        print("No images found. Exiting.")
        return

    random.shuffle(images)
    print(f"Found {len(images)} images. Starting the loop...")

    # Start the first instance
    current_image = images.pop()
    print(current_image)
    proc = subprocess.Popen([BINARY_PATH, current_image])

    while images:
        time.sleep(INTERVAL_SECONDS)
        image_path = images.pop()
        print(image_path)

        # Try to send the new path to the socket
        if not send_to_socket(image_path):
            print("Instance not responding, restarting...")
            # If the process died, clean up and restart
            proc.terminate()
            proc = subprocess.Popen([BINARY_PATH, image_path])

    print("No more images left.")


if __name__ == "__main__":
    main()
