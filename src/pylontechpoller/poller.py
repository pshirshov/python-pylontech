import argparse
import logging
import sys
import time

from pylontech import ExscriptTelnetTransport, Pylontech
from pylontechpoller.hass_basic_reporter import HassReporter
from pylontechpoller.mongo_reporter import MongoReporter
from pylontechpoller.mqtt_reporter import MqttReporter
from pylontechpoller.tools import minimize

logger = logging.getLogger(__name__)


def run(argv: list[str]):
    parser = argparse.ArgumentParser(description="Pylontech RS485 poller")

    parser.add_argument("source_host", help="Telnet host")

    parser.add_argument("--source-port", type=int, help="Telnet host", default=23)
    parser.add_argument("--timeout", type=int, help="timeout", default=2)
    parser.add_argument("--interval", type=int, help="polling interval in msec", default=1000)
    parser.add_argument("--retention-days", type=int, help="how long to retain history data", default=90)
    parser.add_argument("--debug", action="store_true", help="verbose output")

    parser.add_argument("--mongo-url", type=str, help="mongodb url", default=None)
    parser.add_argument("--mongo-db", type=str, help="target mongo database", default="pylontech")
    parser.add_argument("--mongo-collection-history", type=str, help="target mongo collection_hist for stack history", default="history")
    parser.add_argument("--mongo-collection-meta", type=str, help="target mongo collection_hist for stack data", default="meta")

    parser.add_argument("--hass-url", type=str, help="hass url", default=None)
    parser.add_argument("--hass-stack-disbalance", type=str, help="state id", default="input_number.stack_disbalance")
    parser.add_argument("--hass-max-battery-disbalance", type=str, help="state id", default="input_number.max_bat_disbalance")
    parser.add_argument("--hass-max-battery-disbalance-id", type=str, help="state id", default="input_text.max_disbalance_id")
    parser.add_argument("--hass-token", type=str, help="hass token or token file", default="/var/run/agenix/hass-token")

    parser.add_argument("--mqtt-host", type=str, help="mqtt host", default=None)
    parser.add_argument("--mqtt-port", type=int, help="mqtt url", default=1883)
    parser.add_argument("--mqtt-user", type=str, help="mqtt login", default="mqtt")
    parser.add_argument("--mqtt-password", type=str, help="mqtt password or password file", default="/var/run/agenix/mqtt-user")

    args = parser.parse_args(argv[1:])

    level = logging.DEBUG if args.debug else logging.INFO
    logging.basicConfig(
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
        datefmt='%m/%d/%Y %I:%M:%S %p',
        level=level,
    )

    cc = 0
    errs = 0
    spinner = ['|', '/', '-', '\\']

    while True:
        reporters = []
        try:
            logger.debug("Preparing client...")
            p = Pylontech(ExscriptTelnetTransport(host=args.source_host, port=args.source_port, timeout=args.timeout))

            if args.mongo_url:
                reporters.append(MongoReporter(
                    args.mongo_url,
                    args.mongo_db,
                    args.mongo_collection_meta,
                    args.mongo_collection_history,
                    args.retention_days,
                ))

            if args.hass_url:
                reporters.append(HassReporter(
                    args.hass_url,
                    args.hass_stack_disbalance,
                    args.hass_max_battery_disbalance,
                    args.hass_max_battery_disbalance_id,
                    args.hass_token,
                ))

            if args.mqtt_host:
                reporters.append(MqttReporter(
                    args.mqtt_host,
                    args.mqtt_port,
                    args.mqtt_user,
                    args.mqtt_password,
                ))

            logger.info("About to start polling...")
            bats = p.scan_for_batteries(2, 10)
            logger.info("Have battery stack data")

            for reporter in reporters:
                reporter.report_meta(bats, p)

            for b in p.poll_parameters(bats.range()):
                cc += 1

                if sys.stdout.isatty():
                    sys.stdout.write('\r' + spinner[cc % len(spinner)])
                    sys.stdout.flush()

                mb = minimize(b)
                for reporter in reporters:
                    reporter.report_state(mb)

                if cc % 1000 == 0:
                    logger.info("Updates submitted since startup: %d", cc)
                    for reporter in reporters:
                        reporter.cleanup()

                time.sleep(args.interval / 1000.0)
                errs = 0
        except (KeyboardInterrupt, SystemExit):
            for reporter in reporters:
                reporter.close()
            return
        except Exception:
            errs += 1
            logger.exception("Exception occurred")
            for reporter in reporters:
                reporter.close()
            if errs > 10:
                logger.error("Too many exceptions in a row, exiting just in case")
                raise SystemExit(1)
            time.sleep(args.interval / 1000.0)


def main():
    run(sys.argv)


if __name__ == "__main__":
    main()
