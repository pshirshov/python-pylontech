use serde_json::{Map, Value, json};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleIdentity {
    pub address: u8,
    pub serial_number: String,
    pub manufacturer_name: String,
    pub device_name: String,
    pub software_version: String,
    pub cell_count: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleState {
    pub address: u8,
    pub cell_voltages: Vec<f64>,
    pub average_bms_temperature_c: f64,
    pub grouped_cells_temperatures_c: Vec<f64>,
    pub current_a: f64,
    pub voltage_v: f64,
    pub power_w: f64,
    pub remaining_capacity_ah: f64,
    pub total_capacity_ah: f64,
    pub cycle_number: u16,
    pub disbalance_v: f64,
    pub soc_ratio: f64,
    pub soc_percent: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemParameters {
    pub cell_high_voltage_limit_v: f64,
    pub cell_low_voltage_limit_v: f64,
    pub cell_under_voltage_limit_v: f64,
    pub charge_high_temperature_limit_c: f64,
    pub charge_low_temperature_limit_c: f64,
    pub charge_current_limit_a: f64,
    pub module_high_voltage_limit_v: f64,
    pub module_low_voltage_limit_v: f64,
    pub module_under_voltage_limit_v: f64,
    pub discharge_high_temperature_limit_c: f64,
    pub discharge_low_temperature_limit_c: f64,
    pub discharge_current_limit_a: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagementStatus {
    pub charge_enable: bool,
    pub discharge_enable: bool,
    pub charge_immediately_2: bool,
    pub charge_immediately_1: bool,
    pub full_charge_request: bool,
    pub should_charge: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ManagementInfo {
    pub charge_voltage_limit_v: f64,
    pub discharge_voltage_limit_v: f64,
    pub charge_current_limit_a: f64,
    pub discharge_current_limit_a: f64,
    pub status: ManagementStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackState {
    pub stack_disbalance_v: f64,
    pub max_module_disbalance_v: f64,
    pub max_module_id: u8,
}

impl ModuleState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: u8,
        cell_voltages: Vec<f64>,
        average_bms_temperature_c: f64,
        grouped_cells_temperatures_c: Vec<f64>,
        current_a: f64,
        voltage_v: f64,
        remaining_capacity_ah: f64,
        total_capacity_ah: f64,
        cycle_number: u16,
    ) -> AppResult<Self> {
        if cell_voltages.is_empty() {
            return Err(AppError::InvalidState(format!(
                "module {address} had no cell voltages"
            )));
        }
        if total_capacity_ah <= 0.0 {
            return Err(AppError::InvalidState(format!(
                "module {address} total capacity must be positive"
            )));
        }

        let min_voltage = cell_voltages.iter().copied().fold(f64::INFINITY, f64::min);
        let max_voltage = cell_voltages
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let disbalance_v = max_voltage - min_voltage;
        let soc_ratio = remaining_capacity_ah / total_capacity_ah;

        Ok(Self {
            address,
            cell_voltages,
            average_bms_temperature_c,
            grouped_cells_temperatures_c,
            current_a,
            voltage_v,
            power_w: current_a * voltage_v,
            remaining_capacity_ah,
            total_capacity_ah,
            cycle_number,
            disbalance_v,
            soc_ratio,
            soc_percent: soc_ratio * 100.0,
        })
    }

    pub fn to_payload(&self) -> Value {
        let mut payload = Map::new();
        payload.insert("address".to_string(), json!(self.address));
        payload.insert("voltage".to_string(), json!(self.voltage_v));
        payload.insert("current".to_string(), json!(self.current_a));
        payload.insert("power".to_string(), json!(self.power_w));
        payload.insert("cycle".to_string(), json!(self.cycle_number));
        payload.insert("soc_ratio".to_string(), json!(self.soc_ratio));
        payload.insert("soc_percent".to_string(), json!(self.soc_percent));
        payload.insert(
            "temperature".to_string(),
            json!(self.average_bms_temperature_c),
        );
        payload.insert(
            "grouped_temperatures".to_string(),
            json!(self.grouped_cells_temperatures_c),
        );
        payload.insert(
            "remaining_capacity".to_string(),
            json!(self.remaining_capacity_ah),
        );
        payload.insert("total_capacity".to_string(), json!(self.total_capacity_ah));
        payload.insert("disbalance".to_string(), json!(self.disbalance_v));

        for (index, voltage) in self.cell_voltages.iter().enumerate() {
            payload.insert(format!("cell_{index}_voltage"), json!(voltage));
        }

        Value::Object(payload)
    }
}

impl SystemParameters {
    pub fn to_payload(&self) -> Value {
        json!({
            "cell_high_voltage_limit": self.cell_high_voltage_limit_v,
            "cell_low_voltage_limit": self.cell_low_voltage_limit_v,
            "cell_under_voltage_limit": self.cell_under_voltage_limit_v,
            "charge_high_temperature_limit": self.charge_high_temperature_limit_c,
            "charge_low_temperature_limit": self.charge_low_temperature_limit_c,
            "charge_current_limit": self.charge_current_limit_a,
            "module_high_voltage_limit": self.module_high_voltage_limit_v,
            "module_low_voltage_limit": self.module_low_voltage_limit_v,
            "module_under_voltage_limit": self.module_under_voltage_limit_v,
            "discharge_high_temperature_limit": self.discharge_high_temperature_limit_c,
            "discharge_low_temperature_limit": self.discharge_low_temperature_limit_c,
            "discharge_current_limit": self.discharge_current_limit_a,
        })
    }
}

impl ManagementInfo {
    pub fn to_payload(&self) -> Value {
        json!({
            "charge_voltage_limit": self.charge_voltage_limit_v,
            "discharge_voltage_limit": self.discharge_voltage_limit_v,
            "charge_current_limit": self.charge_current_limit_a,
            "discharge_current_limit": self.discharge_current_limit_a,
            "charge_enable": self.status.charge_enable,
            "discharge_enable": self.status.discharge_enable,
            "charge_immediately_2": self.status.charge_immediately_2,
            "charge_immediately_1": self.status.charge_immediately_1,
            "full_charge_request": self.status.full_charge_request,
            "should_charge": self.status.should_charge,
        })
    }
}

impl StackState {
    pub fn from_modules(modules: &[ModuleState]) -> AppResult<Self> {
        if modules.is_empty() {
            return Err(AppError::InvalidState(
                "cannot derive stack state from zero modules".to_string(),
            ));
        }

        let mut global_min = f64::INFINITY;
        let mut global_max = f64::NEG_INFINITY;
        let mut max_module_id = 0;
        let mut max_module_disbalance = f64::NEG_INFINITY;

        for module in modules {
            let module_min = module
                .cell_voltages
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min);
            let module_max = module
                .cell_voltages
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            let module_disbalance = module_max - module_min;

            global_min = global_min.min(module_min);
            global_max = global_max.max(module_max);

            if module_disbalance > max_module_disbalance {
                max_module_disbalance = module_disbalance;
                max_module_id = module.address;
            }
        }

        Ok(Self {
            stack_disbalance_v: global_max - global_min,
            max_module_disbalance_v: max_module_disbalance,
            max_module_id,
        })
    }

    pub fn to_payload(&self) -> Value {
        json!({
            "stack_disbalance": self.stack_disbalance_v,
            "max_module_disbalance": self.max_module_disbalance_v,
            "max_module_id": self.max_module_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ManagementInfo, ManagementStatus, ModuleState, StackState, SystemParameters};
    use serde_json::json;

    #[test]
    fn stack_state_uses_global_cell_span_and_module_disbalance() {
        let first = ModuleState::new(
            2,
            vec![3.31, 3.30],
            24.0,
            vec![24.0],
            -4.0,
            50.0,
            40.0,
            50.0,
            100,
        )
        .unwrap();
        let second = ModuleState::new(
            3,
            vec![3.29, 3.28],
            25.0,
            vec![25.0],
            -3.0,
            49.8,
            39.0,
            50.0,
            101,
        )
        .unwrap();

        let stack = StackState::from_modules(&[first, second]).unwrap();

        assert_eq!(stack.max_module_id, 2);
        assert!((stack.max_module_disbalance_v - 0.01).abs() < 1e-9);
        assert!((stack.stack_disbalance_v - 0.03).abs() < 1e-9);
    }

    #[test]
    fn system_parameters_payload_uses_expected_keys() {
        let payload = SystemParameters {
            cell_high_voltage_limit_v: 3.7,
            cell_low_voltage_limit_v: 3.05,
            cell_under_voltage_limit_v: 2.9,
            charge_high_temperature_limit_c: 33.4,
            charge_low_temperature_limit_c: 26.2,
            charge_current_limit_a: 10.2,
            module_high_voltage_limit_v: 54.0,
            module_low_voltage_limit_v: 46.0,
            module_under_voltage_limit_v: 44.5,
            discharge_high_temperature_limit_c: 33.4,
            discharge_low_temperature_limit_c: 26.2,
            discharge_current_limit_a: -10.0,
        }
        .to_payload();

        assert_eq!(payload["module_high_voltage_limit"], json!(54.0));
        assert_eq!(payload["discharge_current_limit"], json!(-10.0));
    }

    #[test]
    fn management_payload_exposes_status_flags() {
        let payload = ManagementInfo {
            charge_voltage_limit_v: 28.4,
            discharge_voltage_limit_v: 23.2,
            charge_current_limit_a: 55.5,
            discharge_current_limit_a: -55.5,
            status: ManagementStatus {
                charge_enable: true,
                discharge_enable: true,
                charge_immediately_2: false,
                charge_immediately_1: false,
                full_charge_request: false,
                should_charge: false,
            },
        }
        .to_payload();

        assert_eq!(payload["charge_enable"], json!(true));
        assert_eq!(payload["discharge_current_limit"], json!(-55.5));
    }
}
