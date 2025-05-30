from time import sleep

from pylontech import *

if __name__ == '__main__':
    iters = 0

    import sys
    from rich import print_json
    import json

    # socat  -v pty,link=/tmp/serial,waitslave tcp:192.168.10.237:23,forever
    if len(sys.argv) < 2:
        print("Usage: python test-tcp.py <serialdev> <iterations>")
        exit(1)

    host = sys.argv[1]
    iterations = sys.argv[2]

    cont = lambda iter: iter < 1
    if iterations == "inf":
        cont = lambda iter: True
    if iterations != "inf":
        cont = lambda iter: iter < int(iterations)

    p = Pylontech(SerialDeviceTransport(serial_port=host, baudrate=115200))
    bats = p.scan_for_batteries(2, 10)
    print("Battery stack:")
    print_json(json.dumps(to_json_serializable(bats)))

    cc = 0

    try:
        for b in p.poll_parameters(bats.range()):
            cc += 1
            if not cont(cc):
                break
            print("System state:")
            print_json(json.dumps(b))
            sleep(0.5)
    except (KeyboardInterrupt, SystemExit):
        exit(0)
    except BaseException as e:
        raise e
