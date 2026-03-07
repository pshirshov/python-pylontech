import pylontechpoller.poller as poller


class _FakeBats:
    def range(self):
        return range(2, 3)


class _FakeTransport:
    def __init__(self, host, port, timeout):
        self.host = host
        self.port = port
        self.timeout = timeout


class _ReporterSpy:
    instances = []

    def __init__(self, *args):
        self.args = args
        self.meta_calls = 0
        self.state_calls = 0
        self.cleanup_calls = 0
        self.close_calls = 0
        _ReporterSpy.instances.append(self)

    def report_meta(self, meta, p):
        self.meta_calls += 1

    def report_state(self, state):
        self.state_calls += 1

    def cleanup(self):
        self.cleanup_calls += 1

    def close(self):
        self.close_calls += 1


class _RetryingPylontech:
    attempts = 0

    def __init__(self, transport):
        _RetryingPylontech.attempts += 1

    def scan_for_batteries(self, start, end):
        if _RetryingPylontech.attempts == 1:
            raise RuntimeError("first attempt fails")
        return _FakeBats()

    def poll_parameters(self, ids):
        yield {
            "max_module_disbalance": (2, 0.01),
            "stack_disbalance": 0.02,
            "modules": [],
        }
        raise KeyboardInterrupt


class _SinglePassPylontech:
    def __init__(self, transport):
        pass

    def scan_for_batteries(self, start, end):
        return _FakeBats()

    def poll_parameters(self, ids):
        raise KeyboardInterrupt
        yield  # pragma: no cover


def test_run_passes_hass_token(monkeypatch):
    _ReporterSpy.instances.clear()

    monkeypatch.setattr(poller, "ExscriptTelnetTransport", _FakeTransport)
    monkeypatch.setattr(poller, "Pylontech", _SinglePassPylontech)
    monkeypatch.setattr(poller, "HassReporter", _ReporterSpy)

    poller.run([
        "poller",
        "battery.local",
        "--hass-url",
        "http://hass.local",
        "--hass-token",
        "token-value",
    ])

    assert len(_ReporterSpy.instances) == 1
    hass = _ReporterSpy.instances[0]
    assert hass.args[0] == "http://hass.local"
    assert hass.args[4] == "token-value"


def test_run_does_not_reuse_reporters_across_retries(monkeypatch):
    _ReporterSpy.instances.clear()
    _RetryingPylontech.attempts = 0

    monkeypatch.setattr(poller, "ExscriptTelnetTransport", _FakeTransport)
    monkeypatch.setattr(poller, "Pylontech", _RetryingPylontech)
    monkeypatch.setattr(poller, "MqttReporter", _ReporterSpy)
    monkeypatch.setattr(poller, "minimize", lambda payload: payload)

    poller.run([
        "poller",
        "battery.local",
        "--mqtt-host",
        "mqtt.local",
    ])

    assert len(_ReporterSpy.instances) == 2
    first, second = _ReporterSpy.instances

    assert first.meta_calls == 0
    assert first.state_calls == 0
    assert first.close_calls == 1

    assert second.meta_calls == 1
    assert second.state_calls == 1
    assert second.close_calls == 1
