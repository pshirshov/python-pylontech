import argparse
import json
import time
import sys
from rich import print_json
import logging
from pylontech import *
logger = logging.getLogger(__name__)


def run(argv: list[str]):
    parser = argparse.ArgumentParser(description="Pylontech RS485 poller")

    parser.add_argument("source_host", help="Telnet host")
    parser.add_argument("--source-port", help="Telnet host", default=23)
    parser.add_argument("--timeout", type=int, help="timeout", default=2)
    parser.add_argument("--interval", type=int, help="polling interval in msec", default=500)
    parser.add_argument("--debug", type=bool, help="verbose output", default=False)

    args = parser.parse_args(argv[1:])

    level = logging.DEBUG if args.debug else logging.INFO
    logging.basicConfig(format='%(asctime)s - %(name)s - %(levelname)s - %(message)s', datefmt='%m/%d/%Y %I:%M:%S %p', level=level)

    print(args)

    cc = 0
    spinner = ['|', '/', '-', '\\']

    while True:
        try:
            logging.debug("Preparing client...")
            p = Pylontech(ExscriptTelnetTransport(host=args.source_host, port=args.source_port, timeout=args.timeout))
            logging.info("About to start polling...")
            bats = p.scan_for_batteries(2, 10)
            logging.info("Have battery stack data")

            #print_json(json.dumps(to_json_serializable(bats)))

            for b in p.poll_parameters(bats.range()):
                #print_json(json.dumps(b))
                cc += 1

                if sys.stdout.isatty():
                    sys.stdout.write('\r' + spinner[cc % len(spinner)])
                    sys.stdout.flush()

            time.sleep(args.interval / 1000.0)
        except (KeyboardInterrupt, SystemExit):
            exit(0)
        except BaseException as e:
            logging.error("Exception occured: %s", e)




def main():
    import sys
    run(sys.argv)

if __name__ == "__main__":
    main()
