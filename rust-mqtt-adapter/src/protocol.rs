use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use thiserror::Error;

use crate::config::SourceConfig;
use crate::model::{
    ManagementInfo, ManagementStatus, ModuleIdentity, ModuleState, SystemParameters,
};
use crate::stats::RuntimeStats;

const FRAME_START_BYTE: u8 = b'~';
const FRAME_END_BYTE: u8 = b'\r';
const PROTOCOL_VERSION: u8 = 0x20;
const COMMAND_GROUP: u8 = 0x46;
const GET_VALUES_SINGLE_COMMAND: u8 = 0x42;
const GET_SYSTEM_PARAMETERS_COMMAND: u8 = 0x47;
const GET_MANUFACTURER_INFO_COMMAND: u8 = 0x51;
const GET_MANAGEMENT_INFO_COMMAND: u8 = 0x92;
const GET_MODULE_SERIAL_NUMBER_COMMAND: u8 = 0x93;
const DEVICE_NAME_LENGTH: usize = 10;
const SOFTWARE_VERSION_LENGTH: usize = 2;
const MODULE_SERIAL_LENGTH: usize = 16;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("timed out waiting for a frame")]
    Timeout,
    #[error("invalid frame: {0}")]
    InvalidFrame(String),
    #[error("checksum mismatch: expected {expected:04X}, actual {actual:04X}")]
    ChecksumMismatch { expected: u16, actual: u16 },
    #[error("info length mismatch: declared {declared}, actual {actual}")]
    InvalidInfoLength { declared: usize, actual: usize },
    #[error("unexpected end of payload while reading {field}")]
    UnexpectedPayloadEnd { field: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManufacturerInfo {
    pub device_name: String,
    pub software_version: String,
    pub manufacturer_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResponseFrame {
    address: u8,
    cid2: u8,
    info: Vec<u8>,
}

pub struct PylontechClient {
    stream: TcpStream,
    stats: Arc<RuntimeStats>,
}

impl PylontechClient {
    pub fn connect(config: &SourceConfig, stats: Arc<RuntimeStats>) -> Result<Self, ProtocolError> {
        let address = format!("{}:{}", config.host, config.port);
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(config.timeout))?;
        stream.set_write_timeout(Some(config.timeout))?;
        Ok(Self { stream, stats })
    }

    pub fn scan_modules(
        &mut self,
        addresses: impl IntoIterator<Item = u8>,
    ) -> Result<Vec<ModuleIdentity>, ProtocolError> {
        let mut modules = Vec::new();

        for address in addresses {
            match self.try_discover_module(address) {
                Ok(Some(module)) => {
                    eprintln!(
                        "discovered module {} serial={} model={}",
                        module.address, module.serial_number, module.device_name
                    );
                    modules.push(module);
                }
                Ok(None) => {
                    eprintln!("no module at address {}", address);
                }
                Err(error) => {
                    eprintln!("failed to probe module {}: {}", address, error);
                }
            }
        }

        Ok(modules)
    }

    pub fn get_values_single(&mut self, address: u8) -> Result<ModuleState, ProtocolError> {
        let frame = self.request(address, GET_VALUES_SINGLE_COMMAND, &[address])?;
        parse_values_single_payload(address, &frame.info)
    }

    pub fn get_system_parameters(
        &mut self,
        address: u8,
    ) -> Result<SystemParameters, ProtocolError> {
        let frame = self.request(address, GET_SYSTEM_PARAMETERS_COMMAND, &[address])?;
        parse_system_parameters_payload(&frame.info)
    }

    pub fn get_management_info(&mut self, address: u8) -> Result<ManagementInfo, ProtocolError> {
        let frame = self.request(address, GET_MANAGEMENT_INFO_COMMAND, &[address])?;
        parse_management_info_payload(&frame.info)
    }

    fn try_discover_module(
        &mut self,
        address: u8,
    ) -> Result<Option<ModuleIdentity>, ProtocolError> {
        let serial_number = match self.try_get_module_serial_number(address)? {
            Some(serial_number) => serial_number,
            None => return Ok(None),
        };
        let manufacturer_info = self.get_manufacturer_info(address)?;
        let module_state = self.get_values_single(address)?;

        Ok(Some(ModuleIdentity {
            address,
            serial_number,
            manufacturer_name: manufacturer_info.manufacturer_name,
            device_name: manufacturer_info.device_name,
            software_version: manufacturer_info.software_version,
            cell_count: module_state.cell_voltages.len() as u8,
        }))
    }

    fn try_get_module_serial_number(
        &mut self,
        address: u8,
    ) -> Result<Option<String>, ProtocolError> {
        match self.request(address, GET_MODULE_SERIAL_NUMBER_COMMAND, &[address]) {
            Ok(frame) => parse_module_serial_number(&frame.info).map(Some),
            Err(ProtocolError::Timeout) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn get_manufacturer_info(&mut self, address: u8) -> Result<ManufacturerInfo, ProtocolError> {
        let frame = self.request(address, GET_MANUFACTURER_INFO_COMMAND, &[address])?;
        parse_manufacturer_info(&frame.info)
    }

    fn request(
        &mut self,
        address: u8,
        command: u8,
        info_bytes: &[u8],
    ) -> Result<ResponseFrame, ProtocolError> {
        let request_frame = encode_command(address, command, info_bytes)?;
        if let Err(error) = self.stream.write_all(&request_frame) {
            self.stats.record_source_error();
            return Err(ProtocolError::Io(error));
        }
        self.stats.record_source_write(request_frame.len());
        if let Err(error) = self.stream.flush() {
            self.stats.record_source_error();
            return Err(ProtocolError::Io(error));
        }
        let raw_frame = self.read_raw_frame()?;
        match parse_response_frame(&raw_frame) {
            Ok(frame) => Ok(frame),
            Err(error) => {
                self.stats.record_source_error();
                Err(error)
            }
        }
    }

    fn read_raw_frame(&mut self) -> Result<Vec<u8>, ProtocolError> {
        let mut started = false;
        let mut frame = Vec::new();

        loop {
            let mut next_byte = [0_u8; 1];
            match self.stream.read_exact(&mut next_byte) {
                Ok(()) => {
                    let byte = next_byte[0];
                    if !started {
                        if byte == FRAME_START_BYTE {
                            started = true;
                            frame.push(byte);
                        }
                        continue;
                    }

                    frame.push(byte);
                    if byte == FRAME_END_BYTE {
                        self.stats.record_source_read(frame.len());
                        return Ok(frame);
                    }
                }
                Err(error) if is_timeout_error(&error) => {
                    self.stats.record_source_timeout();
                    return Err(ProtocolError::Timeout);
                }
                Err(error) => {
                    self.stats.record_source_error();
                    return Err(ProtocolError::Io(error));
                }
            }
        }
    }
}

fn is_timeout_error(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::TimedOut || error.kind() == std::io::ErrorKind::WouldBlock
}

fn encode_command(address: u8, command: u8, info_bytes: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let info_hex = hex::encode_upper(info_bytes);
    let info_length = encode_info_length(info_hex.len())?;
    let frame_body = format!(
        "{PROTOCOL_VERSION:02X}{address:02X}{COMMAND_GROUP:02X}{command:02X}{info_length:04X}{info_hex}"
    );
    let checksum = frame_checksum(frame_body.as_bytes());
    Ok(format!("~{frame_body}{checksum:04X}\r").into_bytes())
}

fn encode_info_length(encoded_info_length: usize) -> Result<u16, ProtocolError> {
    if encoded_info_length > 0x0FFF {
        return Err(ProtocolError::InvalidFrame(format!(
            "encoded info too large: {} bytes",
            encoded_info_length
        )));
    }
    if encoded_info_length == 0 {
        return Ok(0);
    }

    let len = encoded_info_length as u16;
    let len_sum = (len & 0xF) + ((len >> 4) & 0xF) + ((len >> 8) & 0xF);
    let len_modulo = len_sum % 16;
    let len_invert_plus_one = 0b1111 - len_modulo + 1;

    Ok((len_invert_plus_one << 12) + len)
}

fn frame_checksum(frame: &[u8]) -> u16 {
    let sum = frame
        .iter()
        .fold(0_u32, |accumulator, byte| accumulator + u32::from(*byte));
    let complement = !sum;
    let wrapped = complement % 0x1_0000;
    (wrapped + 1) as u16
}

fn parse_response_frame(raw_frame: &[u8]) -> Result<ResponseFrame, ProtocolError> {
    if raw_frame.len() < 1 + 12 + 4 + 1 {
        return Err(ProtocolError::InvalidFrame(format!(
            "frame too short: {} bytes",
            raw_frame.len()
        )));
    }
    if raw_frame[0] != FRAME_START_BYTE {
        return Err(ProtocolError::InvalidFrame(
            "frame did not start with '~'".to_string(),
        ));
    }
    if *raw_frame.last().unwrap() != FRAME_END_BYTE {
        return Err(ProtocolError::InvalidFrame(
            "frame did not end with carriage return".to_string(),
        ));
    }

    let frame_data = &raw_frame[1..raw_frame.len() - 5];
    let checksum_bytes = &raw_frame[raw_frame.len() - 5..raw_frame.len() - 1];
    let expected_checksum = parse_hex_u16(checksum_bytes)?;
    let actual_checksum = frame_checksum(frame_data);
    if expected_checksum != actual_checksum {
        return Err(ProtocolError::ChecksumMismatch {
            expected: expected_checksum,
            actual: actual_checksum,
        });
    }

    let version = parse_hex_u8(&frame_data[0..2])?;
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::InvalidFrame(format!(
            "unexpected protocol version {version:02X}"
        )));
    }

    let address = parse_hex_u8(&frame_data[2..4])?;
    let cid1 = parse_hex_u8(&frame_data[4..6])?;
    if cid1 != COMMAND_GROUP {
        return Err(ProtocolError::InvalidFrame(format!(
            "unexpected command group {cid1:02X}"
        )));
    }
    let cid2 = parse_hex_u8(&frame_data[6..8])?;
    let info_length = parse_hex_u16(&frame_data[8..12])?;
    let info_hex = &frame_data[12..];
    validate_info_length(info_length, info_hex.len())?;
    let info = hex::decode(info_hex)?;

    Ok(ResponseFrame {
        address,
        cid2,
        info,
    })
}

fn validate_info_length(
    info_length: u16,
    actual_encoded_length: usize,
) -> Result<(), ProtocolError> {
    let declared_length = usize::from(info_length & 0x0FFF);
    if declared_length != actual_encoded_length {
        return Err(ProtocolError::InvalidInfoLength {
            declared: declared_length,
            actual: actual_encoded_length,
        });
    }

    if declared_length == 0 {
        return Ok(());
    }

    let expected = encode_info_length(declared_length)?;
    if expected != info_length {
        return Err(ProtocolError::InvalidFrame(format!(
            "invalid length checksum nibble in {info_length:04X}"
        )));
    }

    Ok(())
}

fn parse_module_serial_number(info: &[u8]) -> Result<String, ProtocolError> {
    let mut payload = ByteCursor::new(info);
    let _command_value = payload.read_u8("CommandValue")?;
    let serial_number = payload.read_exact("ModuleSerialNumber", MODULE_SERIAL_LENGTH)?;
    payload.ensure_exhausted()?;
    Ok(normalize_ascii(serial_number))
}

fn parse_values_single_payload(
    expected_address: u8,
    info: &[u8],
) -> Result<ModuleState, ProtocolError> {
    if info.is_empty() {
        return Err(ProtocolError::InvalidFrame(
            "values payload was empty".to_string(),
        ));
    }

    let mut payload = ByteCursor::new(&info[1..]);

    let reported_address = payload.read_u8("NumberOfModule")?;
    if reported_address != expected_address {
        return Err(ProtocolError::InvalidFrame(format!(
            "requested module {expected_address} but response reported {reported_address}"
        )));
    }

    let cell_count = payload.read_u8("NumberOfCells")?;
    let mut cell_voltages = Vec::with_capacity(cell_count as usize);
    for _ in 0..cell_count {
        cell_voltages.push(f64::from(payload.read_i16("CellVoltage")?) / 1000.0);
    }

    let temperature_count = payload.read_u8("NumberOfTemperatures")?;
    if temperature_count == 0 {
        return Err(ProtocolError::InvalidFrame(format!(
            "module {expected_address} reported zero temperatures"
        )));
    }

    let average_bms_temperature_c =
        kelvin_tenths_to_celsius(payload.read_i16("AverageBMSTemperature")?);

    let mut grouped_cells_temperatures_c = Vec::with_capacity(temperature_count as usize - 1);
    for _ in 0..usize::from(temperature_count - 1) {
        grouped_cells_temperatures_c.push(kelvin_tenths_to_celsius(
            payload.read_i16("GroupedCellTemperature")?,
        ));
    }

    let current_a = f64::from(payload.read_i16("Current")?) / 10.0;
    let voltage_v = f64::from(payload.read_u16("Voltage")?) / 1000.0;
    let remaining_capacity_primary = f64::from(payload.read_u16("RemainingCapacity1")?) / 1000.0;
    let user_defined_items = payload.read_u8("UserDefinedItems")?;
    let total_capacity_primary = f64::from(payload.read_u16("TotalCapacity1")?) / 1000.0;
    let cycle_number = payload.read_u16("CycleNumber")?;

    let (remaining_capacity_ah, total_capacity_ah) = if user_defined_items > 2 {
        (
            f64::from(payload.read_u24("RemainingCapacity2")?) / 1000.0,
            f64::from(payload.read_u24("TotalCapacity2")?) / 1000.0,
        )
    } else {
        (remaining_capacity_primary, total_capacity_primary)
    };

    payload.ensure_exhausted()?;

    ModuleState::new(
        reported_address,
        cell_voltages,
        average_bms_temperature_c,
        grouped_cells_temperatures_c,
        current_a,
        voltage_v,
        remaining_capacity_ah,
        total_capacity_ah,
        cycle_number,
    )
    .map_err(|error| ProtocolError::InvalidFrame(error.to_string()))
}

fn parse_system_parameters_payload(info: &[u8]) -> Result<SystemParameters, ProtocolError> {
    if info.is_empty() {
        return Err(ProtocolError::InvalidFrame(
            "system parameters payload was empty".to_string(),
        ));
    }

    let mut payload = ByteCursor::new(&info[1..]);
    let system_parameters = SystemParameters {
        cell_high_voltage_limit_v: f64::from(payload.read_u16("CellHighVoltageLimit")?) / 1000.0,
        cell_low_voltage_limit_v: f64::from(payload.read_u16("CellLowVoltageLimit")?) / 1000.0,
        cell_under_voltage_limit_v: f64::from(payload.read_i16("CellUnderVoltageLimit")?) / 1000.0,
        charge_high_temperature_limit_c: kelvin_tenths_to_celsius(
            payload.read_i16("ChargeHighTemperatureLimit")?,
        ),
        charge_low_temperature_limit_c: kelvin_tenths_to_celsius(
            payload.read_i16("ChargeLowTemperatureLimit")?,
        ),
        charge_current_limit_a: f64::from(payload.read_i16("ChargeCurrentLimit")?) / 10.0,
        module_high_voltage_limit_v: f64::from(payload.read_u16("ModuleHighVoltageLimit")?)
            / 1000.0,
        module_low_voltage_limit_v: f64::from(payload.read_u16("ModuleLowVoltageLimit")?) / 1000.0,
        module_under_voltage_limit_v: f64::from(payload.read_u16("ModuleUnderVoltageLimit")?)
            / 1000.0,
        discharge_high_temperature_limit_c: kelvin_tenths_to_celsius(
            payload.read_i16("DischargeHighTemperatureLimit")?,
        ),
        discharge_low_temperature_limit_c: kelvin_tenths_to_celsius(
            payload.read_i16("DischargeLowTemperatureLimit")?,
        ),
        discharge_current_limit_a: f64::from(payload.read_i16("DischargeCurrentLimit")?) / 10.0,
    };
    payload.ensure_exhausted()?;
    Ok(system_parameters)
}

fn parse_management_info_payload(info: &[u8]) -> Result<ManagementInfo, ProtocolError> {
    if info.is_empty() {
        return Err(ProtocolError::InvalidFrame(
            "management info payload was empty".to_string(),
        ));
    }

    let mut payload = ByteCursor::new(&info[1..]);
    let charge_voltage_limit_v = f64::from(payload.read_u16("ChargeVoltageLimit")?) / 1000.0;
    let discharge_voltage_limit_v = f64::from(payload.read_u16("DischargeVoltageLimit")?) / 1000.0;
    let charge_current_limit_a = f64::from(payload.read_i16("ChargeCurrentLimit")?) / 10.0;
    let discharge_current_limit_a = f64::from(payload.read_i16("DischargeCurrentLimit")?) / 10.0;
    let status_byte = payload.read_u8("Status")?;
    payload.ensure_exhausted()?;

    let charge_immediately_2 = status_byte & 0b0010_0000 != 0;
    let charge_immediately_1 = status_byte & 0b0001_0000 != 0;
    let full_charge_request = status_byte & 0b0000_1000 != 0;

    Ok(ManagementInfo {
        charge_voltage_limit_v,
        discharge_voltage_limit_v,
        charge_current_limit_a,
        discharge_current_limit_a,
        status: ManagementStatus {
            charge_enable: status_byte & 0b1000_0000 != 0,
            discharge_enable: status_byte & 0b0100_0000 != 0,
            charge_immediately_2,
            charge_immediately_1,
            full_charge_request,
            should_charge: charge_immediately_2 || charge_immediately_1 || full_charge_request,
        },
    })
}

fn parse_manufacturer_info(info: &[u8]) -> Result<ManufacturerInfo, ProtocolError> {
    let mut payload = ByteCursor::new(info);
    let device_name = normalize_ascii(payload.read_exact("DeviceName", DEVICE_NAME_LENGTH)?);
    let version_bytes = payload.read_exact("SoftwareVersion", SOFTWARE_VERSION_LENGTH)?;
    let manufacturer_name = normalize_ascii(payload.read_remaining());

    let software_version = version_bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".");

    Ok(ManufacturerInfo {
        device_name,
        software_version,
        manufacturer_name,
    })
}

fn parse_hex_u8(value: &[u8]) -> Result<u8, ProtocolError> {
    if value.len() != 2 {
        return Err(ProtocolError::InvalidFrame(format!(
            "expected 2 hex digits, got {}",
            value.len()
        )));
    }
    Ok(u8::from_str_radix(
        std::str::from_utf8(value)
            .map_err(|error| ProtocolError::InvalidFrame(error.to_string()))?,
        16,
    )
    .map_err(|error| ProtocolError::InvalidFrame(error.to_string()))?)
}

fn parse_hex_u16(value: &[u8]) -> Result<u16, ProtocolError> {
    if value.len() != 4 {
        return Err(ProtocolError::InvalidFrame(format!(
            "expected 4 hex digits, got {}",
            value.len()
        )));
    }
    Ok(u16::from_str_radix(
        std::str::from_utf8(value)
            .map_err(|error| ProtocolError::InvalidFrame(error.to_string()))?,
        16,
    )
    .map_err(|error| ProtocolError::InvalidFrame(error.to_string()))?)
}

fn kelvin_tenths_to_celsius(value: i16) -> f64 {
    (f64::from(value) - 2731.0) / 10.0
}

fn normalize_ascii(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_matches(char::from(0))
        .trim()
        .to_string()
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u8(&mut self, field: &'static str) -> Result<u8, ProtocolError> {
        let bytes = self.read_exact(field, 1)?;
        Ok(bytes[0])
    }

    fn read_u16(&mut self, field: &'static str) -> Result<u16, ProtocolError> {
        let bytes = self.read_exact(field, 2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_i16(&mut self, field: &'static str) -> Result<i16, ProtocolError> {
        let bytes = self.read_exact(field, 2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u24(&mut self, field: &'static str) -> Result<u32, ProtocolError> {
        let bytes = self.read_exact(field, 3)?;
        Ok((u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]))
    }

    fn read_exact(
        &mut self,
        field: &'static str,
        length: usize,
    ) -> Result<&'a [u8], ProtocolError> {
        if self.offset + length > self.bytes.len() {
            return Err(ProtocolError::UnexpectedPayloadEnd { field });
        }

        let start = self.offset;
        let end = start + length;
        self.offset = end;
        Ok(&self.bytes[start..end])
    }

    fn read_remaining(&mut self) -> &'a [u8] {
        let start = self.offset;
        self.offset = self.bytes.len();
        &self.bytes[start..]
    }

    fn ensure_exhausted(&self) -> Result<(), ProtocolError> {
        if self.offset != self.bytes.len() {
            return Err(ProtocolError::InvalidFrame(format!(
                "payload had {} unexpected trailing bytes",
                self.bytes.len() - self.offset
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        encode_command, parse_management_info_payload, parse_module_serial_number,
        parse_response_frame, parse_system_parameters_payload, parse_values_single_payload,
    };

    #[test]
    fn encode_command_matches_python_frame_format() {
        let encoded = encode_command(2, 0x42, &[0xFF]).unwrap();
        assert_eq!(encoded, b"~20024642E002FFFD09\r");
    }

    #[test]
    fn parse_single_module_values_frame() {
        let raw = b"~20024600D05E1002080D020D020D020D030D000D010D010D03050B7D0B690B690B690B73FFFA680EFFFF04FFFF00000174E401B198E906\r";
        let frame = parse_response_frame(raw).unwrap();
        let state = parse_values_single_payload(2, frame.info.as_slice()).unwrap();

        assert_eq!(state.address, 2);
        assert_eq!(state.cell_voltages.len(), 8);
        assert!((state.cell_voltages[0] - 3.33).abs() < 1e-9);
        assert!((state.average_bms_temperature_c - 21.0).abs() < 1e-9);
        assert!((state.current_a + 0.6).abs() < 1e-9);
        assert!((state.voltage_v - 26.638).abs() < 1e-9);
        assert_eq!(state.cycle_number, 0);
        assert!((state.remaining_capacity_ah - 95.460).abs() < 1e-9);
        assert!((state.total_capacity_ah - 111.0).abs() < 1e-9);
        assert!((state.soc_ratio - 0.86).abs() < 1e-9);
    }

    #[test]
    fn parse_module_serial_payload() {
        let info = [
            0x10, b'S', b'E', b'R', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0',
            b'A', b'B', b'C',
        ];
        let serial = parse_module_serial_number(&info).unwrap();
        assert_eq!(serial, "SER1234567890ABC");
    }

    #[test]
    fn parse_system_parameters_payload_uses_python_scaling() {
        let info = [
            0x01, 0x0E, 0x74, 0x0B, 0xEA, 0x0B, 0x54, 0x0B, 0xF9, 0x0B, 0xB1, 0x00, 0x66, 0xD2,
            0xF0, 0xB3, 0xB0, 0xAD, 0xD4, 0x0B, 0xF9, 0x0B, 0xB1, 0xFF, 0x9C,
        ];
        let system_parameters = parse_system_parameters_payload(&info).unwrap();

        assert!((system_parameters.cell_high_voltage_limit_v - 3.7).abs() < 1e-9);
        assert!((system_parameters.cell_low_voltage_limit_v - 3.05).abs() < 1e-9);
        assert!((system_parameters.cell_under_voltage_limit_v - 2.9).abs() < 1e-9);
        assert!((system_parameters.charge_high_temperature_limit_c - 33.4).abs() < 1e-9);
        assert!((system_parameters.charge_low_temperature_limit_c - 26.2).abs() < 1e-9);
        assert!((system_parameters.charge_current_limit_a - 10.2).abs() < 1e-9);
        assert!((system_parameters.module_high_voltage_limit_v - 54.0).abs() < 1e-9);
        assert!((system_parameters.module_low_voltage_limit_v - 46.0).abs() < 1e-9);
        assert!((system_parameters.module_under_voltage_limit_v - 44.5).abs() < 1e-9);
        assert!((system_parameters.discharge_high_temperature_limit_c - 33.4).abs() < 1e-9);
        assert!((system_parameters.discharge_low_temperature_limit_c - 26.2).abs() < 1e-9);
        assert!((system_parameters.discharge_current_limit_a + 10.0).abs() < 1e-9);
    }

    #[test]
    fn parse_management_payload_uses_python_bit_layout() {
        let raw = b"~20024600B014026EF05AA0022BFDD5C0F915\r";
        let frame = parse_response_frame(raw).unwrap();
        let management_info = parse_management_info_payload(frame.info.as_slice()).unwrap();

        assert!((management_info.charge_voltage_limit_v - 28.4).abs() < 1e-9);
        assert!((management_info.discharge_voltage_limit_v - 23.2).abs() < 1e-9);
        assert!((management_info.charge_current_limit_a - 55.5).abs() < 1e-9);
        assert!((management_info.discharge_current_limit_a + 55.5).abs() < 1e-9);
        assert!(management_info.status.charge_enable);
        assert!(management_info.status.discharge_enable);
        assert!(!management_info.status.charge_immediately_2);
        assert!(!management_info.status.charge_immediately_1);
        assert!(!management_info.status.full_charge_request);
        assert!(!management_info.status.should_charge);
    }
}
