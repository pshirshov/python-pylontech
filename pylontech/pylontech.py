from transport import *

logger = logging.getLogger(__name__)

class PylontechModule:
    def __init__(self, idx, serial, manufacturer_info, device_name, system_parameters, management_info, fw_version):
        self.idx = idx
        self.serial = serial
        self.manufacturer_info = manufacturer_info
        self.device_name = device_name
        self.system_parameters = system_parameters
        self.management_info = management_info
        self.fw_version = fw_version

class PylontechStackData:
    def __init__(self, modules: Dict[int, PylontechModule]):
        self.ids = list(modules.keys())
        self.modules = modules

    def range(self):
        return range(min(self.ids), max(self.ids)+1)

class Pylontech(PylontechSchema):
    def __init__(self, transport):
        self.transport = transport

    def scan_for_batteries(self, start=0, end=255) -> PylontechStackData:
        """ Returns a map of the batteries id to their serial number """
        batteries = {}
        for adr in range(start, end, 1):
            self.transport.send_cmd(adr, 0x93, "{:02X}".format(adr).encode()) # Probe for serial number
            raw_frame = self.transport.readln()

            if raw_frame:
                sn = self.get_module_serial_number(adr)
                sn_str = sn["ModuleSerialNumber"].decode()

                sp = self.get_system_parameters(adr)
                mi = self.get_management_info(adr)

                m = self.get_manufacturer_info(adr)
                nme = m["DeviceName"].decode()
                mfr = m["ManufacturerName"].decode()
                sw = m["SoftwareVersion"]

                batteries[adr] = PylontechModule(adr, sn_str, mfr, nme, sp, mi, sw)

                logger.debug("Found battery at address " + str(adr) + " with serial " + sn_str)
            else:
                logger.debug("No battery found at address " + str(adr))

        return PylontechStackData(batteries)


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
        ff = self.management_info_fmt.parse(f.info[1:])
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

if __name__ == '__main__':
    iters = 0

    import sys
    import datetime
    from rich import print_json

    stop = lambda iter: iter < 1
    if len(sys.argv) > 1 and sys.argv[1] == "inf":
        stop = lambda iter: True
    if len(sys.argv) > 1 and sys.argv[1] != "inf":
        stop = lambda iter: iter < int(sys.argv[1])

    while stop(iters):
        iters += 1
        try:
            p = Pylontech(TelnetTransport(host='192.168.10.237'))
            bats = p.scan_for_batteries(2, 10)
            print("Battery stack:")
            print_json(json.dumps(to_json_serializable(bats)))

            subiters = 0

            while stop(subiters):
                subiters += 1
                result = { "timestamp": datetime.datetime.now().isoformat(), "modules": []}
                for idx in bats.range():
                        vals=to_json_serializable(p.get_values_single(idx))
                        result["modules"].append(vals)
                print("Parameters:")
                print_json(json.dumps(result))

        except (KeyboardInterrupt, SystemExit):
            exit(0)
        except BaseException as e:
            raise e
