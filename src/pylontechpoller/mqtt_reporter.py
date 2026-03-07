import os.path

import paho.mqtt.client as mqtt
from ha_mqtt_discoverable import DeviceInfo, Settings
from ha_mqtt_discoverable.sensors import Sensor, SensorInfo

from pylontech.pylontech import Pylontech, PylontechStackData
from pylontechpoller.reporter import Reporter
from pylontechpoller.tools import minimize


class MqttReporter(Reporter):
    def __init__(self, mqtt_host, mqtt_port, mqtt_login, mqtt_password):
        if os.path.exists(mqtt_password):
            with open(mqtt_password, 'r') as file:
                mqtt_password = file.read().strip()

        self.client = mqtt.Client(client_id="pylontech-poller")
        self.client.username_pw_set(mqtt_login, mqtt_password)
        self.client.connect(mqtt_host, mqtt_port)
        self.client.loop_start()
        self.mqtt_settings = Settings.MQTT(client=self.client)

        self.device_info = DeviceInfo(name="Pylontech Battery Stack", identifiers="pylontech_battery_stack")

        self.hass_stack_disbalance_info = SensorInfo(
            name="Stack Disbalance",
            device_class="voltage",
            unique_id="stack_disbalance",
            unit_of_measurement="V",
            state_class="measurement",
            suggested_display_precision=3,
            device=self.device_info,
            icon="mdi:scale-unbalanced",
        )
        self.hass_stack_disbalance_settings = Settings(mqtt=self.mqtt_settings, entity=self.hass_stack_disbalance_info)
        self.hass_stack_disbalance = Sensor(self.hass_stack_disbalance_settings)

        self.hass_max_battery_disbalance_info = SensorInfo(
            name="Max Battery Disbalance",
            device_class="voltage",
            unique_id="max_battery_disbalance",
            unit_of_measurement="V",
            state_class="measurement",
            suggested_display_precision=3,
            device=self.device_info,
            icon="mdi:scale-unbalanced",
        )
        self.hass_max_battery_disbalance_settings = Settings(
            mqtt=self.mqtt_settings,
            entity=self.hass_max_battery_disbalance_info,
        )
        self.hass_max_battery_disbalance = Sensor(self.hass_max_battery_disbalance_settings)

        self.hass_max_disbalance_id = Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
            name="Max disbalance ID",
            unique_id="max_battery_disbalance_id",
            device=self.device_info,
            icon="mdi:battery-alert",
        )))
        self.bats = {}

    def report_meta(self, meta: PylontechStackData, p: Pylontech):
        moduledata = {m["n"]: m for m in minimize(next(p.poll_parameters(meta.range())))["modules"]}

        for module_id in meta.ids:
            m = meta.modules[module_id]
            device_info = DeviceInfo(
                name=f"Pylontech Battery {module_id}",
                identifiers=[f"pylontech_battery_{m.serial}", f"pylontech_battery_{module_id}"],
                manufacturer=m.manufacturer_info,
                sw_version=".".join([str(x) for x in m.fw_version]),
                model=m.device_name,
            )
            mdata = moduledata[module_id]
            cells = {}
            for cn, _ in enumerate(mdata["cv"]):
                cells[f"cell_{cn}_voltage"] = Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name=f"Cell {cn} Voltage",
                    device_class="voltage",
                    unique_id=f"cell_voltage_{module_id}_{cn}",
                    unit_of_measurement="V",
                    state_class="measurement",
                    suggested_display_precision=3,
                    device=device_info,
                    entity_category="diagnostic",
                    icon="mdi:gauge",
                )))

            self.bats[module_id] = {
                "bat_soc": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="SoC",
                    device_class="battery",
                    unique_id=f"battery_soc_{module_id}",
                    unit_of_measurement="%",
                    state_class="measurement",
                    suggested_display_precision=1,
                    device=device_info,
                ))),
                "bat_disbalance": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Cell Disbalance",
                    device_class="voltage",
                    unique_id=f"battery_disbalance_{module_id}",
                    unit_of_measurement="V",
                    state_class="measurement",
                    suggested_display_precision=3,
                    device=device_info,
                    icon="mdi:scale-unbalanced",
                ))),
                "bat_voltage": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Voltage",
                    device_class="voltage",
                    unique_id=f"battery_voltage_{module_id}",
                    unit_of_measurement="V",
                    state_class="measurement",
                    suggested_display_precision=3,
                    device=device_info,
                    icon="mdi:gauge",
                ))),
                "bat_current": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Current",
                    device_class="current",
                    unique_id=f"battery_current_{module_id}",
                    unit_of_measurement="A",
                    state_class="measurement",
                    suggested_display_precision=3,
                    device=device_info,
                    icon="mdi:current-dc",
                ))),
                "bat_power": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Power",
                    device_class="power",
                    unique_id=f"battery_power_{module_id}",
                    unit_of_measurement="W",
                    state_class="measurement",
                    suggested_display_precision=2,
                    device=device_info,
                    icon="mdi:battery-charging",
                ))),
                "bat_cycle": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Cycle",
                    unique_id=f"battery_cycle_{module_id}",
                    state_class="measurement",
                    device=device_info,
                    icon="mdi:battery-sync",
                ))),
                "bat_temp": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Temperature",
                    device_class="temperature",
                    unique_id=f"battery_temperature_{module_id}",
                    unit_of_measurement="°C",
                    state_class="measurement",
                    suggested_display_precision=1,
                    device=device_info,
                ))),
            } | cells

    def report_state(self, state):
        md = state["max_module_disbalance"]
        self.hass_stack_disbalance.set_state(state["stack_disbalance"])
        self.hass_max_battery_disbalance.set_state(md[1])
        self.hass_max_disbalance_id.set_state(md[0])

        for b in state["modules"]:
            s = self.bats[b["n"]]
            s["bat_disbalance"].set_state(b["disbalance"])
            s["bat_voltage"].set_state(b["v"])
            s["bat_current"].set_state(b["current"])
            s["bat_soc"].set_state(round(b["soc"] * 100, 1))
            s["bat_power"].set_state(b["pw"])
            s["bat_cycle"].set_state(b["cycle"])
            s["bat_temp"].set_state(b["tempavg"])
            for cn, c in enumerate(b["cv"]):
                s[f"cell_{cn}_voltage"].set_state(c)

    def close(self):
        self.client.loop_stop()
        self.client.disconnect()
