from pylontech import *

if __name__ == '__main__':
    """
    Direct TCP connection to devices like Waveshare RS485 to ETH, are 20-50 times faster than 
    serial port emulation through socat. Turn "RFC2217" option on.
    """
    iters = 0

    import sys
    import datetime
    from rich import print_json
    import json

    if len(sys.argv) < 2:
        print("Usage: python test-tcp.py <telnet host> <iterations>")
        exit(1)

    host = sys.argv[1]
    iterations = sys.argv[2]
    stop = lambda iter: iter < 1
    if iterations == "inf":
        stop = lambda iter: True
    if iterations != "inf":
        stop = lambda iter: iter < int(iterations)

    while stop(iters):
        iters += 1
        try:
            p = Pylontech(TelnetTransport(host=host, port=23))
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
