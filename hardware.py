from waferbot.robot import Robot
from waferbot.config import RobotConfig, MotorConfig
from waferbot.process import ProcessGuard

guard = ProcessGuard()
robot = None

def initialize():
    global robot

    if guard.stop_requested():
        raise RuntimeError("Stop is latched")

    guard.acquire_ownership("three_file_pid")

    try:
        config = RobotConfig(motor=MotorConfig(max_speed=140))
        robot = Robot.connected(
            config,
            stop_check=guard.stop_requested,
            bus_guard=guard.bus_guard
        )
        robot.arm()
    except:
        cleanup()
        raise

def read_IR_sensors():
    return robot.read_line_sensors().normalized

def set_motors(FL, FR):
    limit = 255

    left = max(-limit, min(limit, round(FL)))
    right = max(-limit, min(limit, round(FR)))

    speeds = (left, 0, right, 0)
    robot.drive_wheels(speeds)

def stop():
    if robot is not None:
        robot.stop()

def cleanup():
    global robot
    try:
        if robot is not None:
            robot.close()
    finally:
        robot = None
        guard.release_ownership()
