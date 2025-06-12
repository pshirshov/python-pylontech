import logging

from pylontech import Pylontech
from pylontech.pylontech import PylontechStackData

logger = logging.getLogger(__name__)

class Reporter:
    def report_meta(self, meta: PylontechStackData, p: Pylontech):
        pass

    def report_state(self, state):
        pass

    def cleanup(self):
        pass


