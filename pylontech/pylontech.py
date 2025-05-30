from typing import Dict
import logging
import serial
import construct
import telnetlib  # replaced pyserial with telnetlib
from tools import *
from schema import PylontechSchema
from transport import *

logger = logging.getLogger(__name__)

class Pylontech(PylontechSchema):
        # self.s = serial.serial_for_url('rfc2217://192.168.10.237:23?logging=debug&ign_set_control&poll_modem')
        # self.s = serial.serial_for_url('socket://192.168.10.237:23?logging=debug')
    def __init__(self, transport):
        self.transport = transport

    def scan_for_batteries(self, start=0, end=255) -> Dict[int, str]:
        """ Returns a map of the batteries id to their serial number """
        batteries = {}
        for adr in range(start, end, 1):
            self.transport.send_cmd(adr, 0x93, "{:02X}".format(adr).encode()) # Probe for serial number
            raw_frame = self.transport.readln()

            if raw_frame:
                sn = self.get_module_serial_number(adr)
                sn_str = sn["ModuleSerialNumber"].decode()

                batteries[adr] = sn_str
                logger.debug("Found battery at address " + str(adr) + " with serial " + sn_str)
            else:
                logger.debug("No battery found at address " + str(adr))

        return batteries


    def get_protocol_version(self, adr):
        self.transport.send_cmd(adr, 0x4f, "{:02X}".format(adr).encode())
        return self.transport.read_frame()

    def get_manufacturer_info(self, adr):
        self.transport.send_cmd(adr, 0x51, "{:02X}".format(adr).encode())
        f = self.transport.read_frame()
        return self.manufacturer_info_fmt.parse(f.info)

    def get_system_parameters(self, adr):
        self.transport.send_cmd(adr, 0x47, "{:02X}".format(adr).encode())
        f = self.transport.read_frame()
        return self.system_parameters_fmt.parse(f.info[1:])

    def get_management_info(self, adr):
        self.transport.send_cmd(adr, 0x92, "{:02X}".format(adr).encode())
        f = self.transport.read_frame()

        print(f.info)
        print(len(f.info))
        ff = self.management_info_fmt.parse(f.info[1:])
        print(ff)
        return ff

    def get_module_serial_number(self, adr):
        self.transport.send_cmd(adr, 0x93, "{:02X}".format(adr).encode())
        f = self.transport.read_frame()
        return self.module_serial_number_fmt.parse(f.info[0:])

    def get_module_software_version(self, adr):
        self.transport.send_cmd(adr, 0x96, "{:02X}".format(adr).encode())
        f = self.transport.read_frame()
        return self.module_software_version_fmt.parse(f.info)

    def get_values(self):
        self.transport.send_cmd(2, 0x42, b'FF')
        f = self.transport.read_frame()
        return self.get_values_fmt.parse(f.info[1:])

    def get_values_single(self, adr):
        self.transport.send_cmd(adr, 0x42, "{:02X}".format(adr).encode())
        f = self.transport.read_frame()
        return self.get_values_single_fmt.parse(f.info[1:])

    def get_alarm_info(self, adr=0):
        self.transport.send_cmd(adr, 0x4f,b'FF')
        return self.transport.read_frame()


import json

def to_json_serializable(obj):
    from io import BytesIO
    from construct import Container
    import base64

    if isinstance(obj, Container):
        return {k: to_json_serializable(v) for k, v in obj.items() if k != "_io"}
    elif isinstance(obj, dict):
        return {k: to_json_serializable(v) for k, v in obj.items() if k != "_io"}
    elif isinstance(obj, list):
        return [to_json_serializable(v) for v in obj]
    elif isinstance(obj, BytesIO):
        return base64.b64encode(obj.getvalue()).decode('utf-8')  # or use .hex()
    elif isinstance(obj, bytes):
        return base64.b64encode(obj).decode('utf-8')  # or use obj.hex()
    else:
        return obj


if __name__ == '__main__':

    # print('Batteries found :')
    # print()

    # print('Protocol :')
    # print(p.get_protocol_version(2))
    # print('Manufacturer :')
    # print(p.get_manufacturer_info(2))

    while True:
        try:
            p = Pylontech(TelnetTransport(host='192.168.10.237'))
            bats = p.scan_for_batteries(2, 10)

            batmodules = {}
            for id, sn in bats.items():
                batmodules[id] = {
                    "serial": sn,
                    "parameters": to_json_serializable(p.get_system_parameters(id))
                }

            print(json.dumps(batmodules, indent=2))
            exit(1)
            while True:
                for idx in range(2, 10):
                        vals=to_json_serializable(p.get_values_single(idx))
                        print(json.dumps(vals, indent=2))
        except (KeyboardInterrupt, SystemExit):
            exit(0)
        except BaseException as e:
            print(e)
