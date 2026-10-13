"""Stationary camera/OLED test: never opens the robot motor controller."""
import time

from hardware import read_apriltag, start_tag_reader, stop_tag_reader
from yahboom_oled import Yahboom_OLED


def main():
    display = Yahboom_OLED(debug=False, bus=1, address=0x3C)
    display.init_oled_process()
    try:
        display.clear()
        display.add_line("APRILTAG TEST", 1)
        display.add_line("SHOW A TAG", 3)
        display.refresh()
        start_tag_reader()
        print("Show a tag36h11 marker to the camera for 10 seconds")
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            tag_id = read_apriltag()
            if tag_id is not None:
                print(f"APRILTAG DETECTED: {tag_id}")
                display.clear()
                display.add_line("APRILTAG DETECTED", 1)
                display.add_line(f"ID: {tag_id}", 3)
                display.refresh()
            time.sleep(0.05)
    finally:
        stop_tag_reader()
        display.close()


if __name__ == "__main__":
    main()
