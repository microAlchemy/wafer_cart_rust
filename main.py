"""Original PID line follower plus AprilTag-commanded, verified edge switches."""

from pid import PID
from hardware import (
    initialize, read_IR_sensors, set_motors, cleanup, stop,
    show_status, start_tag_reader, read_apriltag,
    clear_tag_events, stop_tag_reader,
)
import time

# UNCHANGED: your verified line-following controller
pid = PID(kp=2, ki=0, kd=1)

BASE = 3
MAX_PWM = 5
DT = 0.002
DURATION = 15

# UNCHANGED: your original BLACK_LEFT edge positions (mm)
EDGE_POSITIONS = {
    (0, 0, 0, 0): -31.25,
    (1, 0, 0, 0): -3.25,
    (1, 1, 0, 0):  0.0,
    (1, 1, 1, 0):  3.25,
    (1, 1, 1, 1): 31.25
}

# NEW: mirror the same position table for BLACK_RIGHT.
# Only used AFTER a successful, confirmed switch to BLACK_RIGHT.
BLACK_RIGHT_POSITIONS = {
    tuple(1 - bit for bit in sensors): position
    for sensors, position in EDGE_POSITIONS.items()
}

# NEW: Example tag IDs; replace with your own tag numbers.
TAG_TO_EDGE = {
    42: "black_right",
    43: "black_left",
}

# NEW: 45-degree, 5-PWM switch tested by you at ~0.8 to 0.9 s.
SWITCH_PWM = 5
SWITCH_MIN_S = 0.70
SWITCH_MAX_S = 0.95  # safety deadline, not a guaranteed crossing time
SWITCH_DT = 0.02     # separate from your unchanged PID DT
SWITCH_CONFIRMATIONS = 3


def edge_visible(sensors, edge):
    """Require an actual black/white boundary; all-0/all-1 aren't proof."""
    pair = (1, 0) if edge == "black_left" else (0, 1)
    return sum(sensors[i:i + 2] == pair for i in range(3)) == 1


def switch_edge(current, target, overall_deadline):
    """Cross diagonally, stop, then independently verify the target edge."""
    if current == target:
        return True

    if (current, target) not in (
        ("black_left", "black_right"),
        ("black_right", "black_left"),
    ):
        return False

    # NEW: Verify the starting edge while stationary.
    for _ in range(SWITCH_CONFIRMATIONS):
        sensors = tuple(read_IR_sensors())
        if not edge_visible(sensors, current):
            print("Switch refused: starting edge not visible:", sensors)
            return False
        time.sleep(SWITCH_DT)

    # NEW: Mecanum DIAGONAL only during the switch.
    # set_motors order: FL, FR, RL, RR (not the waferbot motor-id order).
    wheels = (
        (0, SWITCH_PWM, SWITCH_PWM, 0)  # BLACK_LEFT -> BLACK_RIGHT
        if current == "black_left"
        else (SWITCH_PWM, 0, 0, SWITCH_PWM)  # reverse direction
    )

    started = time.monotonic()
    deadline = min(started + SWITCH_MAX_S, overall_deadline)
    consecutive = 0
    found = False

    try:
        while time.monotonic() < deadline:
            set_motors(*wheels)
            time.sleep(min(SWITCH_DT, max(0, deadline - time.monotonic())))
            read_apriltag()  # Surface camera failures, including during crossing.

            # Don't mistake an immediate glimpse of the opposite boundary
            # for completion of the calibrated diagonal crossing.
            if time.monotonic() - started >= SWITCH_MIN_S:
                sensors = tuple(read_IR_sensors())
                if edge_visible(sensors, target):
                    consecutive += 1
                else:
                    consecutive = 0
                if consecutive >= SWITCH_CONFIRMATIONS:
                    found = True
                    break
    finally:
        stop()  # Always stop before returning, including on errors.

    if not found:
        print("Switch failed: target edge not confirmed before timeout")
        return False

    # NEW: Verify the target edge again while wheels are stopped.
    for _ in range(SWITCH_CONFIRMATIONS):
        sensors = tuple(read_IR_sensors())
        if not edge_visible(sensors, target):
            print("Switch failed: target edge lost while stopped:", sensors)
            return False
        time.sleep(SWITCH_DT)

    print(f"Switch verified: {current} -> {target}")
    return True


if __name__ == "__main__":
    failed = False
    try:
        # NEW: Manual starting edge; AprilTags command subsequent switches.
        edge = input("Initial edge (black_left/black_right): ").strip().lower()
        if edge not in ("black_left", "black_right"):
            raise SystemExit("Invalid starting edge")

        if input("Type yes to start motors: ").strip() != "yes":
            raise SystemExit("Cancelled")

        initialize()
        start_tag_reader()
        show_status("FOLLOWING", edge.upper())

        # UNCHANGED: original PID timing
        start = previous = time.monotonic()
        resume_display_at = None

        while time.monotonic() - start < DURATION:
            cycle = time.monotonic()
            dt = max(0.001, cycle - previous)
            previous = cycle

            # NEW: Show normal edge following after temporary event display.
            if resume_display_at is not None and cycle >= resume_display_at:
                show_status("FOLLOWING", edge.upper())
                resume_display_at = None

            # NEW: Only a known AprilTag may request an edge switch.
            tag_id = read_apriltag()
            if tag_id is not None:
                target = TAG_TO_EDGE.get(tag_id)
                print(f"AprilTag {tag_id}: {target or 'unassigned'}")

                if target is None:
                    show_status(f"TAG {tag_id}", "NOT ASSIGNED")
                    resume_display_at = time.monotonic() + 1.0
                elif target == edge:
                    show_status(f"TAG {tag_id}", "EDGE UNCHANGED")
                    resume_display_at = time.monotonic() + 1.0
                else:
                    stop()
                    show_status(f"TAG {tag_id} DETECTED", f"SWITCH {target.upper()}")
                    if not switch_edge(edge, target, start + DURATION):
                        show_status("SWITCH FAILED", "MOTORS STOPPED")
                        failed = True
                        break

                    edge = target
                    pid.reset()  # NEW: Clear accumulated history after a switch.
                    clear_tag_events()
                    show_status("SWITCH OK", edge.upper())
                    resume_display_at = time.monotonic() + 1.0
                    previous = time.monotonic()
                    continue

            # UNCHANGED: BLACK_LEFT uses your ORIGINAL lookup table.
            sensors = tuple(read_IR_sensors())
            if edge == "black_left":
                position = EDGE_POSITIONS.get(sensors)
            else:
                position = BLACK_RIGHT_POSITIONS.get(sensors)  # NEW

            if position is None:
                stop()
                print("Edge lost:", sensors)
                show_status("EDGE LOST", edge.upper())  # NEW
                failed = True
                break

            # UNCHANGED: your exact PID equation, gain application and clamp
            # Positive edge position means strafe right
            error = position
            u = pid.update(error, dt)
            u = max(-3, min(3, u))

            # UNCHANGED: your exact differential mixing
            FL = BASE + u
            FR = BASE - u
            RL = BASE + u
            RR = BASE - u

            set_motors(FL, FR, RL, RR)
            time.sleep(max(0, DT - (time.monotonic() - cycle)))

        if not failed:
            stop()
            show_status("RUN FINISHED", edge.upper())

    except KeyboardInterrupt:
        stop()
        show_status("STOPPED", "INTERRUPTED")
    except Exception:
        stop()
        try:
            show_status("FAULT", "MOTORS STOPPED")
        except Exception:
            pass
        raise
    finally:
        try:
            stop()
        finally:
            try:
                stop_tag_reader()
            finally:
                cleanup()
