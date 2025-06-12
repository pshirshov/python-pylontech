import datetime

from pymongo import MongoClient

from pylontech import to_json_serializable, Pylontech
from pylontech.pylontech import PylontechStackData
from pylontechpoller.reporter import Reporter


class MongoReporter(Reporter):
    def __init__(self, mongo_url, mongo_db, mongo_collection_meta, mongo_collection_history, retention_days):
        mongo = MongoClient(mongo_url)
        db = mongo[mongo_db]
        self.retention_days = retention_days
        self.collection_meta = db[mongo_collection_meta]
        self.collection_hist = db[mongo_collection_history]
        self.collection_hist.create_index("ts", expireAfterSeconds=3600 * 24 * 90)

    def report_meta(self, meta: PylontechStackData, p: Pylontech):
        self.collection_meta.insert_one({'ts':  datetime.datetime.now().isoformat(), "stack": to_json_serializable(meta)})

    def report_state(self, state):
        self.collection_hist.insert_one(state)

    def cleanup(self):
        threshold = datetime.datetime.now() - datetime.timedelta(days= self.retention_days)
        self.collection_hist.delete_many({"ts": {"$lt": threshold}})
