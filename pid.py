class PID:
    """Filtered-derivative PID; matching the previously used controller."""

    def __init__(self, kp, ki, kd):
        self.kp = kp
        self.ki = ki
        self.kd = kd
        self.reset()

    def reset(self):
        self.integral = 0.0
        self.derivative = 0.0
        self.prev_error = None

    def update(self, error, dt):
        if dt <= 0:
            raise ValueError("dt must be positive")

        self.integral = max(-20, min(20, self.integral + error * dt))
        raw_d = 0 if self.prev_error is None else (error - self.prev_error) / dt
        self.derivative = 0.8 * self.derivative + 0.2 * raw_d
        self.prev_error = error

        return (self.kp * error
                + self.ki * self.integral
                + self.kd * self.derivative)
