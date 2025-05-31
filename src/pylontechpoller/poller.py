import argparse
import datetime
import json
import time
import sys
import logging
import itertools

from rich import print_json
from pylontech import *
from pymongo import MongoClient

logger = logging.getLogger(__name__)


def find_min_max_modules(modules):
    all_voltages = []
    for module in modules:
        for voltage in module["CellVoltages"]:
            all_voltages.append((module["NumberOfModule"], voltage))

    if not all_voltages:
        return None, None

    min_pair = min(all_voltages, key=lambda x: x[1])
    max_pair = max(all_voltages, key=lambda x: x[1])

    return min_pair, max_pair



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

    (min_pair, max_pair) = find_min_max_modules(modules)
    # allcv = list(itertools.chain.from_iterable(map(lambda m: m["CellVoltages"], modules)))
    # vmin = min(allcv)
    # vmax = max(allcv)

    return {
        "ts": b["timestamp"],
        "cvmin": min_pair,
        "cvmax": max_pair,
        "stack_disbalance": min_pair[1] - max_pair[1],
        "modules": list(map(minimize_module, modules)),
    }

def run(argv: list[str]):
    parser = argparse.ArgumentParser(description="Pylontech RS485 poller")

    parser.add_argument("source_host", help="Telnet host")
    
    parser.add_argument("--source-port", help="Telnet host", default=23)
    parser.add_argument("--timeout", type=int, help="timeout", default=2)
    parser.add_argument("--interval", type=int, help="polling interval in msec", default=1)
    parser.add_argument("--debug", type=bool, help="verbose output", default=False)
    parser.add_argument("--mongo-url", type=str, help="mongodb url", default=False)
    parser.add_argument("--mongo-db", type=str, help="target mongo database", default="pylontech")
    parser.add_argument("--mongo-collection-history", type=str, help="target mongo collection_hist for stack history", default="history")
    parser.add_argument("--mongo-collection-meta", type=str, help="target mongo collection_hist for stack data", default="meta")

    args = parser.parse_args(argv[1:])

    level = logging.DEBUG if args.debug else logging.INFO
    logging.basicConfig(format='%(asctime)s - %(name)s - %(levelname)s - %(message)s', datefmt='%m/%d/%Y %I:%M:%S %p', level=level)

    cc = 0
    spinner = ['|', '/', '-', '\\']

    while True:
        try:
            logging.debug("Preparing client...")
            p = Pylontech(ExscriptTelnetTransport(host=args.source_host, port=args.source_port, timeout=args.timeout))
            
            mongo = MongoClient(args.mongo_url)
            db = mongo[args.mongo_db]

            collection_meta = db[args.mongo_collection_meta]

            collection_hist = db[args.mongo_collection_history]
            collection_hist.create_index("createdAt", expireAfterSeconds=3600*24*90)

            logging.info("About to start polling...")
            bats = p.scan_for_batteries(2, 10)

            logging.info("Have battery stack data")
            collection_meta.insert_one({'ts':  datetime.datetime.now().isoformat(), "stack": to_json_serializable(bats)})

            for b in p.poll_parameters(bats.range()):
                cc += 1
                
                if sys.stdout.isatty():
                    sys.stdout.write('\r' + spinner[cc % len(spinner)])
                    sys.stdout.flush()

                # print(print_json(json.dumps(minimize(b))))
                collection_hist.insert_one(minimize(b))
                
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
