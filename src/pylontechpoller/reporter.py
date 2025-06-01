import datetime
import json
import logging

import requests
from pymongo import MongoClient

from pylontech import to_json_serializable

logger = logging.getLogger(__name__)

class Reporter:
    def report_meta(self, meta):
        pass

    def report_state(self, state):
        pass

    def cleanup(self):
        pass

class MongoReporter(Reporter):
    def __init__(self, mongo_url, mongo_db, mongo_collection_meta, mongo_collection_history, retention_days):
        mongo = MongoClient(mongo_url)
        db = mongo[mongo_db]
        self.retention_days = retention_days
        self.collection_meta = db[mongo_collection_meta]
        self.collection_hist = db[mongo_collection_history]
        self.collection_hist.create_index("ts", expireAfterSeconds=3600 * 24 * 90)



    def report_meta(self, meta):
        self.collection_meta.insert_one({'ts':  datetime.datetime.now().isoformat(), "stack": to_json_serializable(meta)})

    def report_state(self, state):
        self.collection_hist.insert_one(state)

    def cleanup(self):
        threshold = datetime.datetime.now() - datetime.timedelta(days= self.retention_days)
        self.collection_hist.delete_many({"ts": {"$lt": threshold}})

class HassReporter(Reporter):
    def __init__(self, hass_url, hass_stack_disbalance, hass_max_battery_disbalance, hass_max_battery_disbalance_id, hass_token_file):
        self.hass_url = hass_url
        self.hass_stack_disbalance = hass_stack_disbalance
        self.hass_max_battery_disbalance = hass_max_battery_disbalance
        self.hass_max_battery_disbalance_id = hass_max_battery_disbalance_id
        with open(hass_token_file, 'r') as file:
            self.hass_token = file.read().strip()


    def report_state(self, state):
        md = state["max_module_disbalance"]
        self.update_hass_state(self.hass_stack_disbalance, int(state["stack_disbalance"] * 10000) / 10000.0)
        self.update_hass_state(self.hass_max_battery_disbalance, int(md[1] * 10000) / 10000.0)
        self.update_hass_state(self.hass_max_battery_disbalance_id, md[0])

    def update_hass_state(self, id, value):
        tpe = id.split('.')[0]
        update = {
            "entity_id": id,
            "value": value
        }

        url = f'{self.hass_url}/api/services/{tpe}/set_value'

        response = requests.post(url, data=json.dumps(update), headers={"Authorization": f"Bearer {self.hass_token}"})

        if response.status_code != 200:
            logger.error(f"hass state update failed for {id}: {response.status_code} {response.text}")
