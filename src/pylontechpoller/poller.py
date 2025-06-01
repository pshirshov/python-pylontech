import argparse
import json
import logging
import sys
import time

from pylontech import *
from pylontechpoller.reporter import MongoReporter, HassReporter

logger = logging.getLogger(__name__)


def find_min_max_modules(modules):
    all_voltages = []
    all_disbalances = []

    for module in modules:
        mid = module["NumberOfModule"]
        cvs = module["CellVoltages"]
        for voltage in cvs:
            all_voltages.append((mid, voltage))
        vmax = max(cvs)
        vmin = min(cvs)
        d = vmax - vmin
        all_disbalances.append((mid, d))

    if not all_voltages:
        return None, None

    min_pair = min(all_voltages, key=lambda x: x[1])
    max_pair = max(all_voltages, key=lambda x: x[1])
    max_disbalance = max(all_disbalances, key=lambda x: abs(x[1]))

    return min_pair, max_pair, max_disbalance



def minimize(b: json) -> json:
    def minimize_module(m: json) -> json:
        return {
            "n": m["NumberOfModule"],
            "v": m["Voltage"],
            "cv": m["CellVoltages"],
            "current": m["Current"],
            "pw": m["Power"],
            "cycle": m["CycleNumber"],
            "soc": m["StateOfCharge"],
            "tempavg": m["AverageBMSTemperature"],
            "temps": m["GroupedCellsTemperatures"],
            "remaining": m["RemainingCapacity"],
            "disbalance": max(m["CellVoltages"]) - min(m["CellVoltages"])
        }

    modules = b["modules"]
    find_min_max_modules(modules)

    (min_pair, max_pair, max_disbalance) = find_min_max_modules(modules)

    return {
        "ts": b["timestamp"],
        "cvmin": min_pair,
        "cvmax": max_pair,
        "stack_disbalance": max_pair[1] - min_pair[1],
        "max_module_disbalance": max_disbalance,
        "modules": list(map(minimize_module, modules)),
    }



def run(argv: list[str]):
    parser = argparse.ArgumentParser(description="Pylontech RS485 poller")

    parser.add_argument("source_host", help="Telnet host")
    
    parser.add_argument("--source-port", help="Telnet host", default=23)
    parser.add_argument("--timeout", type=int, help="timeout", default=2)
    parser.add_argument("--interval", type=int, help="polling interval in msec", default=1000)
    parser.add_argument("--retention-days", type=int, help="how long to retain history data", default=90)
    parser.add_argument("--debug", type=bool, help="verbose output", default=False)

    parser.add_argument("--mongo-url", type=str, help="mongodb url", default=None)
    parser.add_argument("--mongo-db", type=str, help="target mongo database", default="pylontech")
    parser.add_argument("--mongo-collection-history", type=str, help="target mongo collection_hist for stack history", default="history")
    parser.add_argument("--mongo-collection-meta", type=str, help="target mongo collection_hist for stack data", default="meta")

    parser.add_argument("--hass-url", type=str, help="hass url", default=None)
    parser.add_argument("--hass-stack-disbalance", type=str, help="state id", default="input_number.stack_disbalance")
    parser.add_argument("--hass-max-battery-disbalance", type=str, help="state id", default="input_number.max_bat_disbalance")
    parser.add_argument("--hass-max-battery-disbalance-id", type=str, help="state id", default="input_text.max_disbalance_id")
    parser.add_argument("--hass-token-file", type=str, help="hass token file", default="/var/run/agenix/hass-token")


    args = parser.parse_args(argv[1:])

    level = logging.DEBUG if args.debug else logging.INFO
    logging.basicConfig(format='%(asctime)s - %(name)s - %(levelname)s - %(message)s', datefmt='%m/%d/%Y %I:%M:%S %p', level=level)

    cc = 0
    spinner = ['|', '/', '-', '\\']

    reporters = []

    while True:
        try:
            logging.debug("Preparing client...")
            p = Pylontech(ExscriptTelnetTransport(host=args.source_host, port=args.source_port, timeout=args.timeout))

            mongo_url = args.mongo_url

            if mongo_url:
                reporters.append(MongoReporter(
                    mongo_url,
                    args.mongo_db,
                    args.mongo_collection_meta,
                    args.mongo_collection_history,
                    args.retention_days
                ))

            hass_url = args.hass_url
            print(hass_url)
            if hass_url:
                reporters.append(HassReporter(
                    hass_url,
                    args.hass_stack_disbalance,
                    args.hass_max_battery_disbalance,
                    args.hass_max_battery_disbalance_id,
                    args.hass_token_file
                ))

            logging.info("About to start polling...")
            bats = p.scan_for_batteries(2, 10)

            logging.info("Have battery stack data")

            for reporter in reporters:
                reporter.report_meta(bats)

            for b in p.poll_parameters(bats.range()):
                cc += 1
                
                if sys.stdout.isatty():
                    sys.stdout.write('\r' + spinner[cc % len(spinner)])
                    sys.stdout.flush()

                mb = minimize(b)
                # print(print_json(json.dumps(minimize(b))))
                for reporter in reporters:
                    reporter.report_state(mb)

                if cc % 86400 == 0:
                    for reporter in reporters:
                        reporter.cleanup()

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
