import construct
from .tools import *

class PylontechSchema:
    manufacturer_info_fmt = construct.Struct(
        "DeviceName" / JoinBytes(construct.Array(10, construct.Byte)),
        "SoftwareVersion" / construct.Array(2, construct.Byte),
        "ManufacturerName" / JoinBytes(construct.GreedyRange(construct.Byte)),
    )

    system_parameters_fmt = construct.Struct(
        "CellHighVoltageLimit" / ToVolt(construct.Int16ub),
        "CellLowVoltageLimit" / ToVolt(construct.Int16ub),
        "CellUnderVoltageLimit" / ToVolt(construct.Int16sb),
        "ChargeHighTemperatureLimit" / ToCelsius(construct.Int16sb),
        "ChargeLowTemperatureLimit" / ToCelsius(construct.Int16sb),
        "ChargeCurrentLimit" / DivideBy10(construct.Int16sb),
        "ModuleHighVoltageLimit" / ToVolt(construct.Int16ub),
        "ModuleLowVoltageLimit" / ToVolt(construct.Int16ub),
        "ModuleUnderVoltageLimit" / ToVolt(construct.Int16ub),
        "DischargeHighTemperatureLimit" / ToCelsius(construct.Int16sb),
        "DischargeLowTemperatureLimit" / ToCelsius(construct.Int16sb),
        "DischargeCurrentLimit" / DivideBy10(construct.Int16sb),
    )

    management_info_fmt = construct.Struct(
        "ChargeVoltageLimit" / DivideBy1000(construct.Int16ub),
        "DischargeVoltageLimit" / DivideBy1000(construct.Int16ub),
        "ChargeCurrentLimit" / ToAmp(construct.Int16sb),
        "DischargeCurrentLimit" / ToAmp(construct.Int16sb),
        "status"
        / construct.BitStruct(
            "ChargeEnable" / construct.Flag,
            "DischargeEnable" / construct.Flag,
            "ChargeImmediately2" / construct.Flag,
            "ChargeImmediately1" / construct.Flag,
            "FullChargeRequest" / construct.Flag,
            "ShouldCharge"
            / construct.Computed(
                lambda this: this.ChargeImmediately2
                | this.ChargeImmediately1
                | this.FullChargeRequest
            ),
            "_padding" / construct.BitsInteger(3),
        ),
    )

    module_serial_number_fmt = construct.Struct(
        "CommandValue" / construct.Byte,
        "ModuleSerialNumber" / JoinBytes(construct.Array(16, construct.Byte)),
    )

    module_software_version_fmt = construct.Struct(
        "CommandValue" / construct.Byte,
        "ModuleSoftwareVersion" / JoinBytes(construct.Array(5, construct.Byte)),
    )

    get_values_fmt = construct.Struct(
        "NumberOfModules" / construct.Byte,
        "Module" / construct.Array(construct.this.NumberOfModules, construct.Struct(
            "NumberOfCells" / construct.Int8ub,
            "CellVoltages" / construct.Array(construct.this.NumberOfCells, ToVolt(construct.Int16sb)),
            "NumberOfTemperatures" / construct.Int8ub,
            "AverageBMSTemperature" / ToCelsius(construct.Int16sb),
            "GroupedCellsTemperatures" / construct.Array(construct.this.NumberOfTemperatures - 1, ToCelsius(construct.Int16sb)),
            "Current" / ToAmp(construct.Int16sb),
            "Voltage" / ToVolt(construct.Int16ub),
            "Power" / construct.Computed(construct.this.Current * construct.this.Voltage),
            "_RemainingCapacity1" / DivideBy1000(construct.Int16ub),
            "_UserDefinedItems" / construct.Int8ub,
            "_TotalCapacity1" / DivideBy1000(construct.Int16ub),
            "CycleNumber" / construct.Int16ub,
            "_OptionalFields" / construct.If(construct.this._UserDefinedItems > 2,
                                           construct.Struct("RemainingCapacity2" / DivideBy1000(construct.Int24ub),
                                                            "TotalCapacity2" / DivideBy1000(construct.Int24ub))),
            "RemainingCapacity" / construct.Computed(lambda this: this._OptionalFields.RemainingCapacity2 if this._UserDefinedItems > 2 else this._RemainingCapacity1),
            "TotalCapacity" / construct.Computed(lambda this: this._OptionalFields.TotalCapacity2 if this._UserDefinedItems > 2 else this._TotalCapacity1),
        )),
        "TotalPower" / construct.Computed(lambda this: sum([x.Power for x in this.Module])),
        "StateOfCharge" / construct.Computed(lambda this: sum([x.RemainingCapacity for x in this.Module]) / sum([x.TotalCapacity for x in this.Module])),

    )
    get_values_single_fmt = construct.Struct(
        "NumberOfModule" / construct.Byte,
        "NumberOfCells" / construct.Int8ub,
        "CellVoltages" / construct.Array(construct.this.NumberOfCells, ToVolt(construct.Int16sb)),
        "NumberOfTemperatures" / construct.Int8ub,
        "AverageBMSTemperature" / ToCelsius(construct.Int16sb),
        "GroupedCellsTemperatures" / construct.Array(construct.this.NumberOfTemperatures - 1, ToCelsius(construct.Int16sb)),
        "Current" / ToAmp(construct.Int16sb),
        "Voltage" / ToVolt(construct.Int16ub),
        "Power" / construct.Computed(construct.this.Current * construct.this.Voltage),
        "_RemainingCapacity1" / DivideBy1000(construct.Int16ub),
        "_UserDefinedItems" / construct.Int8ub,
        "_TotalCapacity1" / DivideBy1000(construct.Int16ub),
        "CycleNumber" / construct.Int16ub,
        "_OptionalFields" / construct.If(construct.this._UserDefinedItems > 2,
                                       construct.Struct("RemainingCapacity2" / DivideBy1000(construct.Int24ub),
                                                        "TotalCapacity2" / DivideBy1000(construct.Int24ub))),
        "RemainingCapacity" / construct.Computed(lambda this: this._OptionalFields.RemainingCapacity2 if this._UserDefinedItems > 2 else this._RemainingCapacity1),
        "TotalCapacity" / construct.Computed(lambda this: this._OptionalFields.TotalCapacity2 if this._UserDefinedItems > 2 else this._TotalCapacity1),
        "TotalPower" / construct.Computed(construct.this.Power),
        "StateOfCharge" / construct.Computed(construct.this.RemainingCapacity / construct.this.TotalCapacity),
    )
