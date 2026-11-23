#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
# SPDX-License-Identifier: Apache-2.0
"""Control build CPU affinity using GNOME idle time and responsiveness proxies."""
import argparse
import dataclasses
import fcntl
import json
import logging
import math
import os
from pathlib import Path
import re
import shlex
import signal
import subprocess
import threading
import time

logger = logging.getLogger(__name__)
LEVELS = ("low", "medium", "high")
IDLE_COMMAND = ["gdbus", "call", "--session", "--dest", "org.gnome.Mutter.IdleMonitor",
                "--object-path", "/org/gnome/Mutter/IdleMonitor/Core", "--method",
                "org.gnome.Mutter.IdleMonitor.GetIdletime", "--timeout", "1"]


class ArgumentDefaultsRawTextHelpFormatter(argparse.ArgumentDefaultsHelpFormatter,
                                           argparse.RawTextHelpFormatter):
    pass


@dataclasses.dataclass
class Sample:
    idle_ms: int | None
    latency_ms: float
    memory_pressure: float | None
    io_pressure: float | None
    available_percent: float | None


@dataclasses.dataclass
class Policy:
    idle_seconds: float = 300
    latency_ms: float = 100
    recovery_seconds: float = 30
    level: str = "low"
    was_idle: bool = False
    bad_samples: int = 0
    healthy_since: float | None = None
    active_max: str = "low"

    def update(self, sample: Sample, now: float) -> tuple[str, str]:
        unknown = (sample.idle_ms is None or sample.memory_pressure is None
                   or sample.io_pressure is None or sample.available_percent is None)
        pressure = ((sample.memory_pressure or 0) >= 1 or (sample.io_pressure or 0) >= 5
                    or (sample.available_percent is not None and sample.available_percent < 10))
        idle = sample.idle_ms is not None and sample.idle_ms >= self.idle_seconds * 1000
        resumed = self.was_idle and not idle
        self.was_idle = idle
        if unknown or pressure or resumed:
            self.level = "low"
            self.bad_samples = 0
            self.healthy_since = None
            return self.level, "unavailable" if unknown else "resource-pressure" if pressure else "input-resumed"
        if idle:
            self.level = "high"
            self.bad_samples = 0
            self.healthy_since = None
            return self.level, "idle"
        self.level = LEVELS[min(LEVELS.index(self.level), LEVELS.index(self.active_max))]
        if sample.latency_ms >= self.latency_ms:
            self.bad_samples += 1
            self.healthy_since = None
            if self.bad_samples >= 2:
                self.level = LEVELS[max(0, LEVELS.index(self.level) - 1)]
                self.bad_samples = 0
            return self.level, "response-delay"
        self.bad_samples = 0
        if sample.latency_ms >= self.latency_ms / 2:
            self.healthy_since = None
            return self.level, "settling"
        if self.healthy_since is None:
            self.healthy_since = now
        elif now - self.healthy_since >= self.recovery_seconds:
            self.level = LEVELS[min(LEVELS.index(self.active_max), LEVELS.index(self.level) + 1)]
            self.healthy_since = now
        return self.level, "responsive"


def execute(command: list[str], environment: dict[str, str], dry_run: bool = False) -> subprocess.CompletedProcess[str] | None:
    displayed = command
    if command[0].endswith("/low-load"):
        displayed = ["env", f"ZED_LOW_LOAD_RUN_DIR={environment['ZED_LOW_LOAD_RUN_DIR']}",
                     f"ZED_LOW_LOAD_AUTO_CONTROL={environment['ZED_LOW_LOAD_AUTO_CONTROL']}", *command]
    logger.debug("%s", shlex.join(displayed))
    if dry_run:
        print(shlex.join(displayed))
        return None
    return subprocess.run(command, env=environment, text=True, capture_output=True, timeout=5, check=False)


def pressure(resource: str) -> float:
    for line in Path(f"/proc/pressure/{resource}").read_text().splitlines():
        if line.startswith("some "):
            fields = dict(field.split("=", 1) for field in line.split()[1:])
            return float(fields["avg10"])
    raise ValueError(f"No some PSI measurement for {resource}")


def available_memory() -> float:
    fields = dict(line.split(":", 1) for line in Path("/proc/meminfo").read_text().splitlines())
    return 100 * int(fields["MemAvailable"].split()[0]) / int(fields["MemTotal"].split()[0])


def observe(environment: dict[str, str], scheduling_delay_ms: float) -> Sample:
    started = time.monotonic()
    idle_ms = None
    try:
        result = execute(IDLE_COMMAND, environment)
        if result is not None and result.returncode == 0:
            match = re.fullmatch(r"\(uint64 ([0-9]+),\)\s*", result.stdout)
            if match:
                idle_ms = int(match.group(1))
    except (OSError, subprocess.TimeoutExpired) as error:
        logger.debug("Idle monitor unavailable: %s", error)
    latency = max(scheduling_delay_ms, (time.monotonic() - started) * 1000)
    try:
        return Sample(idle_ms, latency, pressure("memory"), pressure("io"), available_memory())
    except (OSError, ValueError, KeyError) as error:
        logger.debug("Resource measurement unavailable: %s", error)
        return Sample(idle_ms, latency, None, None, None)


def runner_alive(directory: Path, repository: Path) -> bool:
    try:
        pid, started, checkout, _affinity = (directory / "runner.tsv").read_text().strip().split("\t")
        if Path(checkout) != repository:
            raise ValueError("Run belongs to another checkout")
        if not pid.isdecimal():
            raise ValueError("Invalid runner PID")
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
        return fields[19] == started and fields[0] != "Z"
    except FileNotFoundError:
        return False


def write_status(directory: Path, **values: object) -> None:
    temporary = directory / "auto-status.json.new"
    temporary.write_text(json.dumps(values, ensure_ascii=False) + "\n")
    temporary.replace(directory / "auto-status.json")


def monitor(args: argparse.Namespace) -> int:
    directory = args.run_directory.resolve(strict=True)
    controller = Path(__file__).resolve().with_name("low-load")
    repository = controller.parent.parent
    environment = dict(os.environ, ZED_LOW_LOAD_RUN_DIR=str(directory), ZED_LOW_LOAD_AUTO_CONTROL="1")
    runtime = Path(environment.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"))
    environment.setdefault("DBUS_SESSION_BUS_ADDRESS", f"unix:path={runtime}/bus")
    if args.dry_run:
        execute(IDLE_COMMAND, environment, True)
        execute([str(controller), "--load", "low"], environment, True)
        return 0
    with (directory / "auto.lock").open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("Automatic controller is already active") from None
        if not runner_alive(directory, repository):
            raise ValueError("Runner is no longer active")
        with (directory / "control.lock").open("w") as control_lock:
            fcntl.flock(control_lock, fcntl.LOCK_EX)
            (directory / "mode").write_text("auto\n")
        policy = Policy(args.idle_seconds, args.latency_ms, args.recovery_seconds, active_max=args.active_max)
        applied = None
        stopped = threading.Event()

        def stop(_signal: int, _frame: object) -> None:
            stopped.set()

        signal.signal(signal.SIGTERM, stop)
        signal.signal(signal.SIGINT, stop)
        next_sample = time.monotonic()
        try:
            while not stopped.is_set() and runner_alive(directory, repository):
                if (directory / "mode").read_text().strip() != "auto":
                    break
                now = time.monotonic()
                sample = observe(environment, max(0, now - next_sample) * 1000)
                level, reason = policy.update(sample, time.monotonic())
                if level != applied:
                    result = execute([str(controller), "--load", level], environment)
                    if result is None:
                        raise RuntimeError("Missing controller result")
                    if result.returncode == 3 or not runner_alive(directory, repository):
                        break
                    if result.returncode != 0:
                        raise RuntimeError(result.stderr.strip() or "CPU affinity change failed")
                    logger.info("Load %s: %s", level, reason)
                    applied = level
                write_status(directory, mode="auto", level=applied, reason=reason, **dataclasses.asdict(sample))
                next_sample = time.monotonic() + args.interval
                stopped.wait(args.interval)
        finally:
            if runner_alive(directory, repository) and (directory / "mode").read_text().strip() == "auto":
                try:
                    result = execute([str(controller), "--load", "low"], environment)
                    if result is not None and result.returncode not in (0, 3):
                        logger.error("Could not restore low load: %s", result.stderr.strip())
                    elif result is not None and result.returncode == 0:
                        applied = "low"
                except (OSError, subprocess.TimeoutExpired) as error:
                    logger.error("Could not restore low load: %s", error)
                with (directory / "control.lock").open("w") as control_lock:
                    fcntl.flock(control_lock, fcntl.LOCK_EX)
                    if (directory / "mode").read_text().strip() == "auto":
                        (directory / "mode").write_text("manual\n")
            write_status(directory, mode="stopped", last_auto_level=applied)
    return 0


def positive(value: str) -> float:
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("Must be a finite positive number")
    return number


def main() -> int:
    parser = argparse.ArgumentParser(formatter_class=ArgumentDefaultsRawTextHelpFormatter)
    parser.add_argument("-q", "--quiet", action="count", default=0)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("monitor", formatter_class=ArgumentDefaultsRawTextHelpFormatter)
    command.add_argument("--run-directory", required=True, type=Path)
    command.add_argument("--idle-seconds", type=positive, default=300)
    command.add_argument("--interval", type=positive, default=1)
    command.add_argument("--latency-ms", type=positive, default=100)
    command.add_argument("--recovery-seconds", type=positive, default=30)
    command.add_argument("--active-max", choices=LEVELS, default="low",
                         help="Maximum load while using the desktop; latency proxy can miss perceived stalls")
    command.add_argument("-n", "--dry_run", "--dry-run", dest="dry_run", action="store_true")
    command.set_defaults(func=monitor)
    args = parser.parse_args()
    logging.basicConfig(level=(logging.DEBUG if args.quiet == 0 else logging.INFO if args.quiet == 1 else logging.WARNING),
                        format="[%(levelname)s %(asctime)s] %(message)s", datefmt="%T")
    try:
        return args.func(args)
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        logger.error("%s", error)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
