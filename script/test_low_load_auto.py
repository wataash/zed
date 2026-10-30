#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
# SPDX-License-Identifier: Apache-2.0
"""Deterministic tests of automatic build-load policy; no desktop changes."""
import importlib.util
from pathlib import Path
import sys
import unittest

spec = importlib.util.spec_from_file_location("low_load_auto", Path(__file__).with_name("low-load-auto.py"))
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)


def sample(idle_ms=0, latency_ms=10, memory_pressure=0, io_pressure=0, available_percent=50):
    return module.Sample(idle_ms, latency_ms, memory_pressure, io_pressure, available_percent)


class PolicyTests(unittest.TestCase):
    def test_five_minute_boundary(self):
        policy = module.Policy()
        self.assertEqual(policy.update(sample(idle_ms=299999), 0)[0], "low")
        self.assertEqual(policy.update(sample(idle_ms=300000), 1), ("high", "idle"))

    def test_input_resumes_at_low(self):
        policy = module.Policy()
        policy.update(sample(idle_ms=300000), 0)
        self.assertEqual(policy.update(sample(), 1), ("low", "input-resumed"))

    def test_gradual_recovery(self):
        policy = module.Policy(active_max="high")
        for timestamp, expected in [(0, "low"), (29, "low"), (30, "medium"), (59, "medium"), (60, "high")]:
            self.assertEqual(policy.update(sample(), timestamp)[0], expected)

    def test_two_slow_samples_lower_one_level(self):
        policy = module.Policy(level="high", active_max="high")
        for timestamp, expected in [(0, "high"), (1, "medium"), (2, "medium"), (3, "low")]:
            self.assertEqual(policy.update(sample(latency_ms=100), timestamp)[0], expected)

    def test_borderline_response_resets_recovery(self):
        policy = module.Policy(active_max="high")
        policy.update(sample(), 0)
        policy.update(sample(latency_ms=60), 29)
        self.assertEqual(policy.update(sample(), 30)[0], "low")
        self.assertEqual(policy.update(sample(), 60)[0], "medium")

    def test_pressure_overrides_idle(self):
        for kwargs in [{"memory_pressure": 1}, {"io_pressure": 5}, {"available_percent": 9}]:
            with self.subTest(**kwargs):
                policy = module.Policy(level="high")
                self.assertEqual(policy.update(sample(idle_ms=300000, **kwargs), 0), ("low", "resource-pressure"))

    def test_missing_measurements_fail_low(self):
        for key in ["idle_ms", "memory_pressure", "io_pressure", "available_percent"]:
            with self.subTest(key=key):
                policy = module.Policy(level="high")
                self.assertEqual(policy.update(sample(**{key: None}), 0), ("low", "unavailable"))

    def test_delay_is_ignored_while_idle(self):
        self.assertEqual(module.Policy().update(sample(idle_ms=300000, latency_ms=1000), 0), ("high", "idle"))

    def test_default_never_increases_while_active(self):
        policy = module.Policy()
        for timestamp in [0, 30, 60, 300, 3600]:
            self.assertEqual(policy.update(sample(), timestamp)[0], "low")

    def test_active_cap_applies_to_existing_high(self):
        self.assertEqual(module.Policy(level="high").update(sample(), 0)[0], "low")

    def test_medium_cap_and_idle_override(self):
        policy = module.Policy(active_max="medium")
        for timestamp, expected in [(0, "low"), (30, "medium"), (60, "medium")]:
            self.assertEqual(policy.update(sample(), timestamp)[0], expected)
        self.assertEqual(policy.update(sample(idle_ms=300000), 61)[0], "high")


if __name__ == "__main__":
    unittest.main()
