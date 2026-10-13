from pid import PID
from hardware import initialize, read_IR_sensors, set_motors, cleanup, stop
import time

pid = PID(kp=0.6, ki=0, kd=0.00)

BASE = 3
MAX_PWM = 5
DT = 0.0002
DURATION = 10

# BLACK_LEFT edge position (mm)
EDGE_POSITIONS = {
    (0, 0, 0, 0): -31.25,
    (1, 0, 0, 0): -3.25,
    (1, 1, 0, 0):  0.0,
    (1, 1, 1, 0):  3.25,
    (1, 1, 1, 1): 31.25
}

if __name__ == "__main__":
    try:
        if input("Type yes to start motors: ").strip() != "yes":
            raise SystemExit("Cancelled")

        initialize()

        start = previous = time.monotonic()

        while time.monotonic() - start < DURATION:
            cycle = time.monotonic()
            dt = max(0.001, cycle - previous)
            previous = cycle

            sensors = tuple(read_IR_sensors())
            position = EDGE_POSITIONS.get(sensors)

            if position is None:
                stop()
                print("Edge lost:", sensors)
                break

            # Positive edge position means strafe right
            error = position
            u = pid.update(error, dt)
            u = max(-3, min(3, u))

            # Differential mixing
            FL = BASE + u
            FR = BASE - u
            RL = BASE + u
            RR = BASE - u

            set_motors(FL, FR, RL, RR)
            time.sleep(max(0, DT - (time.monotonic() - cycle)))

    finally:
        cleanup()