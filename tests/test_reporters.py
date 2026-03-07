from types import SimpleNamespace

import pylontechpoller.mongo_reporter as mongo_reporter
import pylontechpoller.mqtt_reporter as mqtt_reporter


class _FakeCollection:
    def __init__(self):
        self.create_index_calls = []

    def create_index(self, *args, **kwargs):
        self.create_index_calls.append((args, kwargs))

    def insert_one(self, _):
        pass

    def delete_many(self, _):
        pass


class _FakeDb:
    def __init__(self):
        self.collections = {
            "meta": _FakeCollection(),
            "hist": _FakeCollection(),
        }

    def __getitem__(self, name):
        return self.collections[name]


class _FakeMongoClient:
    def __init__(self, _url):
        self.db = _FakeDb()

    def __getitem__(self, _name):
        return self.db


class _FakeStateSensor:
    def __init__(self):
        self.values = []

    def set_state(self, value):
        self.values.append(value)


def test_mongo_reporter_uses_configured_retention_for_ttl(monkeypatch):
    monkeypatch.setattr(mongo_reporter, "MongoClient", _FakeMongoClient)

    reporter = mongo_reporter.MongoReporter(
        "mongodb://localhost:27017",
        "pylontech",
        "meta",
        "hist",
        17,
    )

    calls = reporter.collection_hist.create_index_calls
    assert len(calls) == 1
    assert calls[0][0] == ("ts",)
    assert calls[0][1]["expireAfterSeconds"] == 17 * mongo_reporter.SECONDS_PER_DAY


def test_mqtt_reporter_soc_is_rounded_not_truncated():
    reporter = object.__new__(mqtt_reporter.MqttReporter)
    reporter.hass_stack_disbalance = _FakeStateSensor()
    reporter.hass_max_battery_disbalance = _FakeStateSensor()
    reporter.hass_max_disbalance_id = _FakeStateSensor()
    reporter.bats = {
        2: {
            "bat_disbalance": _FakeStateSensor(),
            "bat_voltage": _FakeStateSensor(),
            "bat_current": _FakeStateSensor(),
            "bat_soc": _FakeStateSensor(),
            "bat_power": _FakeStateSensor(),
            "bat_cycle": _FakeStateSensor(),
            "bat_temp": _FakeStateSensor(),
            "cell_0_voltage": _FakeStateSensor(),
        }
    }

    reporter.report_state({
        "stack_disbalance": 0.12,
        "max_module_disbalance": (2, 0.03),
        "modules": [{
            "n": 2,
            "disbalance": 0.03,
            "v": 50.1,
            "current": -4.2,
            "soc": 0.67895,
            "pw": -210.42,
            "cycle": 101,
            "tempavg": 24.4,
            "cv": [3.31],
        }],
    })

    assert reporter.bats[2]["bat_soc"].values == [67.9]


def test_mqtt_reporter_uses_state_class_and_celsius_unit(monkeypatch):
    sensor_info_calls = []

    class _FakeClient:
        def username_pw_set(self, *_args):
            pass

        def connect(self, *_args):
            pass

        def loop_start(self):
            pass

        def loop_stop(self):
            pass

        def disconnect(self):
            pass

    class _FakeMqttModule:
        @staticmethod
        def Client(client_id):
            assert client_id == "pylontech-poller"
            return _FakeClient()

    class _FakeSettings:
        def __init__(self, mqtt, entity):
            self.mqtt = mqtt
            self.entity = entity

        @staticmethod
        def MQTT(client):
            return SimpleNamespace(client=client)

    class _FakeSensor:
        def __init__(self, settings):
            self.settings = settings

        def set_state(self, _value):
            pass

    def _fake_sensor_info(**kwargs):
        sensor_info_calls.append(kwargs)
        return SimpleNamespace(**kwargs)

    monkeypatch.setattr(mqtt_reporter, "mqtt", _FakeMqttModule)
    monkeypatch.setattr(mqtt_reporter, "Settings", _FakeSettings)
    monkeypatch.setattr(mqtt_reporter, "Sensor", _FakeSensor)
    monkeypatch.setattr(mqtt_reporter, "SensorInfo", _fake_sensor_info)
    monkeypatch.setattr(mqtt_reporter, "DeviceInfo", lambda **kwargs: SimpleNamespace(**kwargs))
    monkeypatch.setattr(mqtt_reporter, "minimize", lambda payload: payload)

    reporter = mqtt_reporter.MqttReporter("mqtt.local", 1883, "user", "pass")

    module = SimpleNamespace(
        serial="SER123",
        manufacturer_info="Pylon",
        fw_version=[1, 2, 3],
        device_name="US2000",
    )
    meta = SimpleNamespace(ids=[2], modules={2: module}, range=lambda: range(2, 3))

    class _FakePylontech:
        def poll_parameters(self, _ids):
            yield {"modules": [{"n": 2, "cv": [3.31]}]}

    reporter.report_meta(meta, _FakePylontech())

    by_unique_id = {entry.get("unique_id"): entry for entry in sensor_info_calls}
    assert by_unique_id["stack_disbalance"]["state_class"] == "measurement"
    assert by_unique_id["battery_soc_2"]["state_class"] == "measurement"
    assert by_unique_id["battery_temperature_2"]["state_class"] == "measurement"
    assert by_unique_id["battery_temperature_2"]["unit_of_measurement"] == "°C"
