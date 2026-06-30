import os
import time
import random
import subprocess

BASE_DIR = "/mnt/struct_1/pics/"
INTERVAL_SECONDS = 300
VALID_EXTENSIONS = {".jpg", ".jpeg", ".png", ".gif"}


def get_all_images(base_dir):
    images = []
    for root, dirs, files in os.walk(base_dir):
        for file in files:
            ext = os.path.splitext(file)[1].lower()
            if ext in VALID_EXTENSIONS:
                images.append(os.path.join(root, file))
    return images


def main():
    images = get_all_images(BASE_DIR)

    if not images:
        print("No images found in the specified directory. Exiting.")
        return

    print(f"Found {len(images)} images. Starting the loop...")
    random.shuffle(images)

    while images:
        image_path = images.pop()
        print(f"Running timba on: {image_path}")

        # This will wait for the command to finish.
        # If timba is a blocking GUI, use subprocess.Popen instead.
        subprocess.run(["target/release/timba", image_path])

        if images:
            time.sleep(INTERVAL_SECONDS)

    print("No more images left. Quitting.")


if __name__ == "__main__":
    main()
