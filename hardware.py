"""Raspbot motor/IR hardware, USB AprilTags, and Yahboom 128x32 OLED."""

import queue
import threading
import time

import cv2
from waferbot.config import MotorConfig, RobotConfig
from waferbot.process import ProcessGuard
from waferbot.robot import Robot
from yahboom_oled import Yahboom_OLED

MAX_PWM = 5
CAMERA_INDEX = 0

guard = ProcessGuard()
robot = None
oled = None
_tag_queue = queue.Queue(maxsize=8)
_tag_stop = threading.Event()
_tag_thread = None
_tag_error = None
_tag_last_frame = 0.0


def initialize():
    global robot, oled
    if guard.stop_requested():
        raise RuntimeError("Robot stop is latched")

    guard.acquire_ownership("wafer_cart_apriltag")
    try:
        # IMPORTANT: Retain any wheel inversions / sensor mapping from your
        # calibrated, currently working hardware.py if they differ from defaults.
        config = RobotConfig(motor=MotorConfig(max_speed=MAX_PWM))
        robot = Robot.connected(
            config, stop_check=guard.stop_requested, bus_guard=guard.bus_guard
        )
        robot.arm()

        # Packaged OLED driver is copied from the user's archived repository.
        oled = Yahboom_OLED(debug=False, bus=1, address=0x3C)
        oled.init_oled_process()
        show_status("WAFER CART", "READY")
    except Exception:
        cleanup()
        raise


def read_IR_sensors():
    return tuple(robot.read_line_sensors().normalized)


def set_motors(FL, FR, RL, RR):
    # Public helper uses FL, FR, RL, RR; waferbot expects FL, RL, FR, RR.
    speeds = tuple(
        max(-MAX_PWM, min(MAX_PWM, round(value)))
        for value in (FL, RL, FR, RR)
    )
    robot.drive_wheels(speeds)


def stop():
    if robot is not None:
        robot.stop()


def show_status(first, second=""):
    if oled is None:
        return
    # 5-pixel glyph + 1-pixel gap => 21 characters per 128px row.
    oled.clear()
    oled.add_line(str(first).upper()[:21], 1)
    oled.add_line(str(second).upper()[:21], 3)
    oled.refresh()


def start_tag_reader():
    global _tag_thread, _tag_error, _tag_last_frame
    if _tag_thread is not None and _tag_thread.is_alive():
        raise RuntimeError("AprilTag reader already running")

    if not hasattr(cv2, "aruco") or not hasattr(cv2.aruco, "DICT_APRILTAG_36h11"):
        raise RuntimeError("OpenCV lacks the AprilTag 36h11 dictionary")

    dictionary = cv2.aruco.getPredefinedDictionary(cv2.aruco.DICT_APRILTAG_36h11)
    detector = (
        cv2.aruco.ArucoDetector(dictionary)
        if hasattr(cv2.aruco, "ArucoDetector") else None
    )
    camera = cv2.VideoCapture(CAMERA_INDEX)
    if not camera.isOpened():
        camera.release()
        raise RuntimeError(f"Cannot open camera {CAMERA_INDEX}")

    while not _tag_queue.empty():
        _tag_queue.get_nowait()
    _tag_error = None
    _tag_last_frame = time.monotonic()
    _tag_stop.clear()

    def scan():
        global _tag_error, _tag_last_frame
        candidate = None
        consecutive = 0
        latched = None
        missing = 0
        try:
            while not _tag_stop.is_set():
                ok, frame = camera.read()
                if not ok or frame is None:
                    raise RuntimeError("Camera stopped returning frames")
                _tag_last_frame = time.monotonic()

                if detector is not None:
                    _, ids, _ = detector.detectMarkers(frame)
                else:
                    _, ids, _ = cv2.aruco.detectMarkers(frame, dictionary)

                # Act only on one unambiguous visible tag.
                tag_id = int(ids[0][0]) if ids is not None and len(ids) == 1 else None

                if tag_id is None:
                    missing += 1
                    if missing >= 10:
                        latched = None  # Permit a new sighting after leaving view
                else:
                    missing = 0

                if tag_id == candidate and tag_id is not None:
                    consecutive += 1
                else:
                    candidate = tag_id
                    consecutive = 1 if tag_id is not None else 0

                if consecutive >= 3 and tag_id != latched:
                    try:
                        _tag_queue.put_nowait(tag_id)
                    except queue.Full:
                        pass
                    latched = tag_id

                time.sleep(0.02)
        except Exception as exc:
            _tag_error = exc
        finally:
            camera.release()

    _tag_thread = threading.Thread(target=scan, name="apriltag", daemon=True)
    _tag_thread.start()


def read_apriltag():
    if _tag_error is not None:
        raise RuntimeError("AprilTag camera failed") from _tag_error
    if _tag_thread is not None and not _tag_thread.is_alive():
        raise RuntimeError("AprilTag reader unexpectedly stopped")
    if _tag_thread is not None and time.monotonic() - _tag_last_frame > 2.0:
        raise RuntimeError("AprilTag camera frames are stale")
    try:
        return _tag_queue.get_nowait()
    except queue.Empty:
        return None


def clear_tag_events():
    while True:
        try:
            _tag_queue.get_nowait()
        except queue.Empty:
            return


def stop_tag_reader():
    if _tag_thread is not None:
        _tag_stop.set()
        _tag_thread.join(timeout=2.0)


def cleanup():
    global robot, oled
    try:
        if robot is not None:
            robot.close()
    finally:
        robot = None
        try:
            if oled is not None:
                oled.close()
        finally:
            oled = None
            guard.release_ownership()
