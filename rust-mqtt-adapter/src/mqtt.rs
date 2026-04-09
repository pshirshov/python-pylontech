use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rumqttc::{Client, LastWill, MqttOptions, QoS};
use serde_json::{Map, Value, json};

use crate::config::MqttConfig;
use crate::error::AppResult;
use crate::model::{ManagementInfo, ModuleIdentity, ModuleState, StackState, SystemParameters};
use crate::stats::RuntimeStats;

const MQTT_KEEPALIVE_SECONDS: u64 = 30;
const MQTT_REQUEST_CAPACITY: usize = 32;

struct SensorDefinition {
    component: &'static str,
    name: &'static str,
    unique_id_prefix: &'static str,
    state_key: &'static str,
    device_class: Option<&'static str>,
    unit_of_measurement: Option<&'static str>,
    state_class: Option<&'static str>,
    icon: Option<&'static str>,
    entity_category: Option<&'static str>,
    suggested_display_precision: Option<u8>,
}

const STACK_SENSORS: [SensorDefinition; 3] = [
    SensorDefinition {
        component: "sensor",
        name: "Stack Disbalance",
        unique_id_prefix: "stack_disbalance",
        state_key: "stack_disbalance",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: Some("measurement"),
        icon: Some("mdi:scale-unbalanced"),
        entity_category: None,
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Max Battery Disbalance",
        unique_id_prefix: "max_battery_disbalance",
        state_key: "max_module_disbalance",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: Some("measurement"),
        icon: Some("mdi:scale-unbalanced"),
        entity_category: None,
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Max Disbalance ID",
        unique_id_prefix: "max_battery_disbalance_id",
        state_key: "max_module_id",
        device_class: None,
        unit_of_measurement: None,
        state_class: None,
        icon: Some("mdi:battery-alert"),
        entity_category: None,
        suggested_display_precision: None,
    },
];

const MODULE_SENSORS: [SensorDefinition; 7] = [
    SensorDefinition {
        component: "sensor",
        name: "SoC",
        unique_id_prefix: "battery_soc",
        state_key: "soc_percent",
        device_class: Some("battery"),
        unit_of_measurement: Some("%"),
        state_class: Some("measurement"),
        icon: None,
        entity_category: None,
        suggested_display_precision: Some(1),
    },
    SensorDefinition {
        component: "sensor",
        name: "Cell Disbalance",
        unique_id_prefix: "battery_disbalance",
        state_key: "disbalance",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: Some("measurement"),
        icon: Some("mdi:scale-unbalanced"),
        entity_category: None,
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Voltage",
        unique_id_prefix: "battery_voltage",
        state_key: "voltage",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: Some("measurement"),
        icon: Some("mdi:gauge"),
        entity_category: None,
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Current",
        unique_id_prefix: "battery_current",
        state_key: "current",
        device_class: Some("current"),
        unit_of_measurement: Some("A"),
        state_class: Some("measurement"),
        icon: Some("mdi:current-dc"),
        entity_category: None,
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Power",
        unique_id_prefix: "battery_power",
        state_key: "power",
        device_class: Some("power"),
        unit_of_measurement: Some("W"),
        state_class: Some("measurement"),
        icon: Some("mdi:battery-charging"),
        entity_category: None,
        suggested_display_precision: Some(2),
    },
    SensorDefinition {
        component: "sensor",
        name: "Cycle",
        unique_id_prefix: "battery_cycle",
        state_key: "cycle",
        device_class: None,
        unit_of_measurement: None,
        state_class: Some("measurement"),
        icon: Some("mdi:battery-sync"),
        entity_category: None,
        suggested_display_precision: None,
    },
    SensorDefinition {
        component: "sensor",
        name: "Temperature",
        unique_id_prefix: "battery_temperature",
        state_key: "temperature",
        device_class: Some("temperature"),
        unit_of_measurement: Some("°C"),
        state_class: Some("measurement"),
        icon: None,
        entity_category: None,
        suggested_display_precision: Some(1),
    },
];

const SYSTEM_PARAMETER_SENSORS: [SensorDefinition; 12] = [
    SensorDefinition {
        component: "sensor",
        name: "Cell High Voltage Limit",
        unique_id_prefix: "battery_cell_high_voltage_limit",
        state_key: "cell_high_voltage_limit",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: None,
        icon: Some("mdi:gauge"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Cell Low Voltage Limit",
        unique_id_prefix: "battery_cell_low_voltage_limit",
        state_key: "cell_low_voltage_limit",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: None,
        icon: Some("mdi:gauge"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Cell Under Voltage Limit",
        unique_id_prefix: "battery_cell_under_voltage_limit",
        state_key: "cell_under_voltage_limit",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: None,
        icon: Some("mdi:gauge"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Charge High Temperature Limit",
        unique_id_prefix: "battery_charge_high_temperature_limit",
        state_key: "charge_high_temperature_limit",
        device_class: Some("temperature"),
        unit_of_measurement: Some("°C"),
        state_class: None,
        icon: Some("mdi:thermometer-high"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(1),
    },
    SensorDefinition {
        component: "sensor",
        name: "Charge Low Temperature Limit",
        unique_id_prefix: "battery_charge_low_temperature_limit",
        state_key: "charge_low_temperature_limit",
        device_class: Some("temperature"),
        unit_of_measurement: Some("°C"),
        state_class: None,
        icon: Some("mdi:thermometer-low"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(1),
    },
    SensorDefinition {
        component: "sensor",
        name: "Charge Current Limit",
        unique_id_prefix: "battery_charge_current_limit",
        state_key: "charge_current_limit",
        device_class: Some("current"),
        unit_of_measurement: Some("A"),
        state_class: None,
        icon: Some("mdi:current-dc"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(1),
    },
    SensorDefinition {
        component: "sensor",
        name: "Module High Voltage Limit",
        unique_id_prefix: "battery_module_high_voltage_limit",
        state_key: "module_high_voltage_limit",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: None,
        icon: Some("mdi:gauge"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Module Low Voltage Limit",
        unique_id_prefix: "battery_module_low_voltage_limit",
        state_key: "module_low_voltage_limit",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: None,
        icon: Some("mdi:gauge"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Module Under Voltage Limit",
        unique_id_prefix: "battery_module_under_voltage_limit",
        state_key: "module_under_voltage_limit",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: None,
        icon: Some("mdi:gauge"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Discharge High Temperature Limit",
        unique_id_prefix: "battery_discharge_high_temperature_limit",
        state_key: "discharge_high_temperature_limit",
        device_class: Some("temperature"),
        unit_of_measurement: Some("°C"),
        state_class: None,
        icon: Some("mdi:thermometer-high"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(1),
    },
    SensorDefinition {
        component: "sensor",
        name: "Discharge Low Temperature Limit",
        unique_id_prefix: "battery_discharge_low_temperature_limit",
        state_key: "discharge_low_temperature_limit",
        device_class: Some("temperature"),
        unit_of_measurement: Some("°C"),
        state_class: None,
        icon: Some("mdi:thermometer-low"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(1),
    },
    SensorDefinition {
        component: "sensor",
        name: "Discharge Current Limit",
        unique_id_prefix: "battery_discharge_current_limit",
        state_key: "discharge_current_limit",
        device_class: Some("current"),
        unit_of_measurement: Some("A"),
        state_class: None,
        icon: Some("mdi:current-dc"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(1),
    },
];

const MANAGEMENT_VALUE_SENSORS: [SensorDefinition; 4] = [
    SensorDefinition {
        component: "sensor",
        name: "Charge Voltage Limit",
        unique_id_prefix: "battery_management_charge_voltage_limit",
        state_key: "charge_voltage_limit",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: None,
        icon: Some("mdi:gauge"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Discharge Voltage Limit",
        unique_id_prefix: "battery_management_discharge_voltage_limit",
        state_key: "discharge_voltage_limit",
        device_class: Some("voltage"),
        unit_of_measurement: Some("V"),
        state_class: None,
        icon: Some("mdi:gauge"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(3),
    },
    SensorDefinition {
        component: "sensor",
        name: "Charge Current Limit",
        unique_id_prefix: "battery_management_charge_current_limit",
        state_key: "charge_current_limit",
        device_class: Some("current"),
        unit_of_measurement: Some("A"),
        state_class: None,
        icon: Some("mdi:current-dc"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(1),
    },
    SensorDefinition {
        component: "sensor",
        name: "Discharge Current Limit",
        unique_id_prefix: "battery_management_discharge_current_limit",
        state_key: "discharge_current_limit",
        device_class: Some("current"),
        unit_of_measurement: Some("A"),
        state_class: None,
        icon: Some("mdi:current-dc"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: Some(1),
    },
];

const MANAGEMENT_FLAG_SENSORS: [SensorDefinition; 6] = [
    SensorDefinition {
        component: "binary_sensor",
        name: "Charge Enable",
        unique_id_prefix: "battery_management_charge_enable",
        state_key: "charge_enable",
        device_class: None,
        unit_of_measurement: None,
        state_class: None,
        icon: Some("mdi:battery-charging"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: None,
    },
    SensorDefinition {
        component: "binary_sensor",
        name: "Discharge Enable",
        unique_id_prefix: "battery_management_discharge_enable",
        state_key: "discharge_enable",
        device_class: None,
        unit_of_measurement: None,
        state_class: None,
        icon: Some("mdi:battery-arrow-down"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: None,
    },
    SensorDefinition {
        component: "binary_sensor",
        name: "Charge Immediately 2",
        unique_id_prefix: "battery_management_charge_immediately_2",
        state_key: "charge_immediately_2",
        device_class: None,
        unit_of_measurement: None,
        state_class: None,
        icon: Some("mdi:flash"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: None,
    },
    SensorDefinition {
        component: "binary_sensor",
        name: "Charge Immediately 1",
        unique_id_prefix: "battery_management_charge_immediately_1",
        state_key: "charge_immediately_1",
        device_class: None,
        unit_of_measurement: None,
        state_class: None,
        icon: Some("mdi:flash"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: None,
    },
    SensorDefinition {
        component: "binary_sensor",
        name: "Full Charge Request",
        unique_id_prefix: "battery_management_full_charge_request",
        state_key: "full_charge_request",
        device_class: None,
        unit_of_measurement: None,
        state_class: None,
        icon: Some("mdi:battery-plus"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: None,
    },
    SensorDefinition {
        component: "binary_sensor",
        name: "Should Charge",
        unique_id_prefix: "battery_management_should_charge",
        state_key: "should_charge",
        device_class: None,
        unit_of_measurement: None,
        state_class: None,
        icon: Some("mdi:battery-heart-variant"),
        entity_category: Some("diagnostic"),
        suggested_display_precision: None,
    },
];

pub struct MqttPublisher {
    client: Client,
    discovery_prefix: String,
    topic_prefix: String,
    healthy: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
    stats: Arc<RuntimeStats>,
}

impl MqttPublisher {
    pub fn connect(config: &MqttConfig, stats: Arc<RuntimeStats>) -> AppResult<Self> {
        let status_topic = format!("{}/status", config.topic_prefix);
        let mut options = MqttOptions::new(&config.client_id, &config.host, config.port);
        options.set_keep_alive(Duration::from_secs(MQTT_KEEPALIVE_SECONDS));
        options.set_last_will(LastWill::new(
            status_topic.clone(),
            "offline",
            QoS::AtLeastOnce,
            true,
        ));

        if let Some(username) = &config.username {
            let password = config.password.clone().unwrap_or_default();
            options.set_credentials(username, password);
        }

        let (client, mut connection) = Client::new(options, MQTT_REQUEST_CAPACITY);
        let healthy = Arc::new(AtomicBool::new(true));
        let last_error = Arc::new(Mutex::new(None));
        let healthy_for_thread = Arc::clone(&healthy);
        let last_error_for_thread = Arc::clone(&last_error);
        thread::spawn(move || {
            let mut failure_message = "mqtt event loop stopped unexpectedly".to_string();
            for notification in connection.iter() {
                if let Err(error) = notification {
                    failure_message = format!("mqtt event loop stopped: {}", error);
                    eprintln!("{}", failure_message);
                    break;
                }
            }
            *last_error_for_thread
                .lock()
                .expect("mqtt last_error mutex must not be poisoned") = Some(failure_message);
            healthy_for_thread.store(false, Ordering::SeqCst);
        });

        let publisher = Self {
            client,
            discovery_prefix: config.discovery_prefix.clone(),
            topic_prefix: config.topic_prefix.clone(),
            healthy,
            last_error,
            stats,
        };
        publisher.publish_text(&publisher.availability_topic(), true, "online")?;
        Ok(publisher)
    }

    pub fn ensure_healthy(&self) -> AppResult<()> {
        if self.healthy.load(Ordering::SeqCst) {
            return Ok(());
        }

        let error_message = self
            .last_error
            .lock()
            .expect("mqtt last_error mutex must not be poisoned")
            .clone()
            .unwrap_or_else(|| "mqtt event loop stopped unexpectedly".to_string());
        Err(crate::error::AppError::MqttDisconnected(error_message))
    }

    pub fn publish_offline_best_effort(&self) {
        if self
            .client
            .publish(self.availability_topic(), QoS::AtLeastOnce, true, "offline")
            .is_ok()
        {
            self.stats.record_mqtt_message_sent();
        }
    }

    pub fn publish_discovery(&self, modules: &[ModuleIdentity]) -> AppResult<()> {
        self.publish_stack_discovery()?;
        for module in modules {
            self.publish_module_discovery(module)?;
        }
        Ok(())
    }

    pub fn publish_stack_state(&self, state: &StackState) -> AppResult<()> {
        self.publish_json(&self.stack_state_topic(), false, &state.to_payload())
    }

    pub fn publish_module_state(&self, state: &ModuleState) -> AppResult<()> {
        self.publish_json(
            &self.module_state_topic(state.address),
            false,
            &state.to_payload(),
        )
    }

    pub fn publish_system_parameters(
        &self,
        address: u8,
        parameters: &SystemParameters,
    ) -> AppResult<()> {
        self.publish_json(
            &self.module_system_topic(address),
            true,
            &parameters.to_payload(),
        )
    }

    pub fn publish_management_info(
        &self,
        address: u8,
        management_info: &ManagementInfo,
    ) -> AppResult<()> {
        self.publish_json(
            &self.module_management_topic(address),
            false,
            &management_info.to_payload(),
        )
    }

    fn publish_stack_discovery(&self) -> AppResult<()> {
        for sensor in STACK_SENSORS {
            let payload = discovery_payload(
                sensor.component,
                sensor.name,
                sensor.unique_id_prefix,
                &self.stack_state_topic(),
                sensor.state_key,
                &self.stack_device(),
                &self.availability_topic(),
                sensor.device_class,
                sensor.unit_of_measurement,
                sensor.state_class,
                sensor.icon,
                sensor.entity_category,
                sensor.suggested_display_precision,
            );
            self.publish_json(
                &self.discovery_topic(sensor.component, sensor.unique_id_prefix),
                true,
                &payload,
            )?;
        }

        Ok(())
    }

    fn publish_module_discovery(&self, module: &ModuleIdentity) -> AppResult<()> {
        let device = self.module_device(module);
        let state_topic = self.module_state_topic(module.address);

        for sensor in MODULE_SENSORS {
            let unique_id = format!("{}_{}", sensor.unique_id_prefix, module.address);
            let payload = discovery_payload(
                sensor.component,
                sensor.name,
                &unique_id,
                &state_topic,
                sensor.state_key,
                &device,
                &self.availability_topic(),
                sensor.device_class,
                sensor.unit_of_measurement,
                sensor.state_class,
                sensor.icon,
                sensor.entity_category,
                sensor.suggested_display_precision,
            );
            self.publish_json(
                &self.discovery_topic(sensor.component, &unique_id),
                true,
                &payload,
            )?;
        }

        for cell_index in 0..module.cell_count {
            let unique_id = format!("cell_voltage_{}_{}", module.address, cell_index);
            let payload = discovery_payload(
                "sensor",
                &format!("Cell {} Voltage", cell_index),
                &unique_id,
                &state_topic,
                &format!("cell_{}_voltage", cell_index),
                &device,
                &self.availability_topic(),
                Some("voltage"),
                Some("V"),
                Some("measurement"),
                Some("mdi:gauge"),
                Some("diagnostic"),
                Some(3),
            );
            self.publish_json(&self.discovery_topic("sensor", &unique_id), true, &payload)?;
        }

        for sensor in SYSTEM_PARAMETER_SENSORS {
            let unique_id = format!("{}_{}", sensor.unique_id_prefix, module.address);
            let payload = discovery_payload(
                sensor.component,
                sensor.name,
                &unique_id,
                &self.module_system_topic(module.address),
                sensor.state_key,
                &device,
                &self.availability_topic(),
                sensor.device_class,
                sensor.unit_of_measurement,
                sensor.state_class,
                sensor.icon,
                sensor.entity_category,
                sensor.suggested_display_precision,
            );
            self.publish_json(
                &self.discovery_topic(sensor.component, &unique_id),
                true,
                &payload,
            )?;
        }

        for sensor in MANAGEMENT_VALUE_SENSORS {
            let unique_id = format!("{}_{}", sensor.unique_id_prefix, module.address);
            let payload = discovery_payload(
                sensor.component,
                sensor.name,
                &unique_id,
                &self.module_management_topic(module.address),
                sensor.state_key,
                &device,
                &self.availability_topic(),
                sensor.device_class,
                sensor.unit_of_measurement,
                sensor.state_class,
                sensor.icon,
                sensor.entity_category,
                sensor.suggested_display_precision,
            );
            self.publish_json(
                &self.discovery_topic(sensor.component, &unique_id),
                true,
                &payload,
            )?;
        }

        for sensor in MANAGEMENT_FLAG_SENSORS {
            let unique_id = format!("{}_{}", sensor.unique_id_prefix, module.address);
            let payload = discovery_payload(
                sensor.component,
                sensor.name,
                &unique_id,
                &self.module_management_topic(module.address),
                sensor.state_key,
                &device,
                &self.availability_topic(),
                sensor.device_class,
                sensor.unit_of_measurement,
                sensor.state_class,
                sensor.icon,
                sensor.entity_category,
                sensor.suggested_display_precision,
            );
            self.publish_json(
                &self.discovery_topic(sensor.component, &unique_id),
                true,
                &payload,
            )?;
        }

        Ok(())
    }

    fn publish_json(&self, topic: &str, retain: bool, payload: &Value) -> AppResult<()> {
        self.publish_text(topic, retain, payload.to_string())
    }

    fn publish_text(
        &self,
        topic: &str,
        retain: bool,
        payload: impl Into<Vec<u8>>,
    ) -> AppResult<()> {
        self.ensure_healthy()?;
        self.client
            .publish(topic, QoS::AtLeastOnce, retain, payload)?;
        self.stats.record_mqtt_message_sent();
        Ok(())
    }

    fn availability_topic(&self) -> String {
        format!("{}/status", self.topic_prefix)
    }

    fn stack_state_topic(&self) -> String {
        format!("{}/stack/state", self.topic_prefix)
    }

    fn module_state_topic(&self, address: u8) -> String {
        format!("{}/module/{}/state", self.topic_prefix, address)
    }

    fn module_system_topic(&self, address: u8) -> String {
        format!("{}/module/{}/system/state", self.topic_prefix, address)
    }

    fn module_management_topic(&self, address: u8) -> String {
        format!("{}/module/{}/management/state", self.topic_prefix, address)
    }

    fn discovery_topic(&self, component: &str, unique_id: &str) -> String {
        format!(
            "{}/{}/{}/config",
            self.discovery_prefix, component, unique_id
        )
    }

    fn stack_device(&self) -> Value {
        json!({
            "name": "Pylontech Battery Stack",
            "identifiers": ["pylontech_battery_stack"],
        })
    }

    fn module_device(&self, module: &ModuleIdentity) -> Value {
        let identifiers = vec![
            format!("pylontech_battery_{}", module.serial_number),
            format!("pylontech_battery_{}", module.address),
        ];
        json!({
            "name": format!("Pylontech Battery {}", module.address),
            "identifiers": identifiers,
            "manufacturer": module.manufacturer_name,
            "model": module.device_name,
            "sw_version": module.software_version,
        })
    }
}

fn discovery_payload(
    component: &str,
    name: &str,
    unique_id: &str,
    state_topic: &str,
    state_key: &str,
    device: &Value,
    availability_topic: &str,
    device_class: Option<&str>,
    unit_of_measurement: Option<&str>,
    state_class: Option<&str>,
    icon: Option<&str>,
    entity_category: Option<&str>,
    suggested_display_precision: Option<u8>,
) -> Value {
    let mut payload = BTreeMap::new();
    payload.insert("name".to_string(), json!(name));
    payload.insert("unique_id".to_string(), json!(unique_id));
    payload.insert("state_topic".to_string(), json!(state_topic));
    payload.insert(
        "value_template".to_string(),
        json!(format!("{{{{ value_json.{state_key} }}}}")),
    );
    payload.insert("availability_topic".to_string(), json!(availability_topic));
    payload.insert("payload_available".to_string(), json!("online"));
    payload.insert("payload_not_available".to_string(), json!("offline"));
    payload.insert("device".to_string(), device.clone());

    insert_optional(&mut payload, "device_class", device_class);
    insert_optional(&mut payload, "unit_of_measurement", unit_of_measurement);
    insert_optional(&mut payload, "state_class", state_class);
    insert_optional(&mut payload, "icon", icon);
    insert_optional(&mut payload, "entity_category", entity_category);

    if component == "binary_sensor" {
        payload.insert("payload_on".to_string(), json!("true"));
        payload.insert("payload_off".to_string(), json!("false"));
    }

    if let Some(precision) = suggested_display_precision {
        payload.insert("suggested_display_precision".to_string(), json!(precision));
    }

    let object = payload.into_iter().collect::<Map<String, Value>>();
    Value::Object(object)
}

fn insert_optional(payload: &mut BTreeMap<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        payload.insert(key.to_string(), json!(value));
    }
}

#[cfg(test)]
mod tests {
    use super::discovery_payload;
    use serde_json::json;

    #[test]
    fn discovery_payload_uses_json_value_template() {
        let payload = discovery_payload(
            "sensor",
            "SoC",
            "battery_soc_2",
            "pylontech/module/2/state",
            "soc_percent",
            &json!({"name": "Pylontech Battery 2"}),
            "pylontech/status",
            Some("battery"),
            Some("%"),
            Some("measurement"),
            None,
            None,
            Some(1),
        );

        assert_eq!(
            payload["value_template"],
            json!("{{ value_json.soc_percent }}")
        );
        assert_eq!(payload["state_topic"], json!("pylontech/module/2/state"));
        assert_eq!(payload["device_class"], json!("battery"));
        assert_eq!(payload["suggested_display_precision"], json!(1));
    }

    #[test]
    fn binary_sensor_payload_uses_true_false() {
        let payload = discovery_payload(
            "binary_sensor",
            "Should Charge",
            "battery_management_should_charge_2",
            "pylontech/module/2/management/state",
            "should_charge",
            &json!({"name": "Pylontech Battery 2"}),
            "pylontech/status",
            None,
            None,
            None,
            None,
            Some("diagnostic"),
            None,
        );

        assert_eq!(payload["payload_on"], json!("true"));
        assert_eq!(payload["payload_off"], json!("false"));
    }
}
