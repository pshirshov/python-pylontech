import os.path

from ha_mqtt_discoverable import Settings, DeviceInfo
from ha_mqtt_discoverable.sensors import SensorInfo, Sensor

from pylontech.pylontech import PylontechModule, Pylontech, PylontechStackData
from pylontechpoller.tools import minimize
from pylontechpoller.reporter import Reporter

import paho.mqtt.client as mqtt


class MqttReporter(Reporter):
    def __init__(self, mqtt_host, mqtt_port, mqtt_login, mqtt_password):
        if os.path.exists(mqtt_password):
            with open(mqtt_password, 'r') as file:
                mqtt_password = file.read().strip()

        client = mqtt.Client(client_id="pylontech-poller")
        client.username_pw_set(mqtt_login, mqtt_password)
        client.connect(mqtt_host, mqtt_port)
        client.loop_start()
        self.mqtt_settings = Settings.MQTT(client=client)
        # client.enable_logger(logger)

        # self.mqtt_settings = Settings.MQTT(host=mqtt_host, port=mqtt_port, username=mqtt_login, password=mqtt_password,
        #                                    client_name="pylontech-poller")

        self.device_info = DeviceInfo(name="Pylontech Battery Stack", identifiers="pylontech_battery_stack")

        self.hass_stack_disbalance_info = SensorInfo(
            name="Stack Disbalance",
            device_class="voltage",
            unique_id="stack_disbalance",
            unit_of_measurement="V",
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
            suggested_display_precision=3,
            device=self.device_info,
            icon="mdi:scale-unbalanced",
        )
        self.hass_max_battery_disbalance_settings = Settings(mqtt=self.mqtt_settings,
                                                             entity=self.hass_max_battery_disbalance_info)
        self.hass_max_battery_disbalance = Sensor(self.hass_max_battery_disbalance_settings)

        self.hass_max_disbalance_id = Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
            name="Max disbalance ID",
            unique_id=f"max_battery_disbalance_id",
            device=self.device_info,
            icon="mdi:battery-alert",

        )))
        self.bats = {}

    def report_meta(self, meta: PylontechStackData, p: Pylontech):
        moduledata = { m["n"] : m for m in minimize( next(p.poll_parameters(meta.range())) )["modules"]}
        cells = {}

        for id in meta.ids:
            m = meta.modules[id]
            device_info = DeviceInfo(
                name=f"Pylontech Battery {id}",
                identifiers=[f"pylontech_battery_{m.serial}", f"pylontech_battery_{id}", ],
                manufacturer=m.manufacturer_info,
                sw_version=".".join([str(x) for x in m.fw_version]),
                model=m.device_name
            )
            mdata = moduledata[id]
            for cn, c in enumerate(mdata["cv"]):
                cells[f"cell_{cn}_voltage"] = Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                        name=f"Cell {cn} Voltage",
                        device_class="voltage",
                        unique_id=f"cell_voltage_{id}_{cn}",
                        unit_of_measurement="V",
                        suggested_display_precision=3,
                        device=device_info,
                        entity_category="diagnostic",
                        icon="mdi:gauge",
                    )))

            self.bats[id] = {
                "bat_soc": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="SoC",
                    device_class="battery",
                    unique_id=f"battery_soc_{id}",
                    unit_of_measurement="%",
                    suggested_display_precision=1,
                    device=device_info
                ))),
                "bat_disbalance": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Cell Disbalance",
                    device_class="voltage",
                    unique_id=f"battery_disbalance_{id}",
                    unit_of_measurement="V",
                    suggested_display_precision=3,
                    device=device_info,
                    icon="mdi:scale-unbalanced",
                ))),
                "bat_voltage": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Voltage",
                    device_class="voltage",
                    unique_id=f"battery_voltage_{id}",
                    unit_of_measurement="V",
                    suggested_display_precision=3,
                    device=device_info,
                    icon="mdi:gauge",
                ))),
                "bat_current": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Current",
                    device_class="current",
                    unique_id=f"battery_current_{id}",
                    unit_of_measurement="A",
                    suggested_display_precision=3,
                    device=device_info,
                    icon="mdi:current-dc",
                ))),
                "bat_power": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Power",
                    device_class="power",
                    unique_id=f"battery_power_{id}",
                    unit_of_measurement="W",
                    suggested_display_precision=2,
                    device=device_info,
                    icon="mdi:battery-charging",
                ))),
                "bat_cycle": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Cycle",
                    unique_id=f"battery_cycle_{id}",
                    device=device_info,
                    icon="mdi:battery-sync",
                ))),
                "bat_temp": Sensor(Settings(mqtt=self.mqtt_settings, entity=SensorInfo(
                    name="Temperature",
                    device_class="temperature",
                    unique_id=f"battery_temperature_{id}",
                    unit_of_measurement="C",
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
            s["bat_soc"].set_state(int(b["soc"] * 1000) / 10.0)
            s["bat_power"].set_state(b["pw"])
            s["bat_cycle"].set_state(b["cycle"])
            s["bat_temp"].set_state(b["tempavg"])
            for cn, c in enumerate(b["cv"]):
                s[f"cell_{cn}_voltage"].set_state(c)