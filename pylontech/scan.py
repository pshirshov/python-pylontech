import pylontech


if __name__ == '__main__':
    p = pylontech.Pylontech()
    print('Batteries found :')
    print(p.scan_for_batteries(2,3))
    print('Protocol :')
    print(p.get_protocol_version(2))
    print('Manufacturer :')
    print(p.get_manufacturer_info(2))
    print('System parameters :')
    print(p.get_system_parameters(2))
    print('Management Info :')
    print(p.get_management_info(2))
    print('Serial Number :')
    print(p.get_module_serial_number(2))
    print('Software version :')
    print(p.get_module_software_version(2))
    #below lines doesn't get battery answer
    #print('Software version :')
    #print(p.get_values())
    print('State :')
    print(p.get_values_single(2))
