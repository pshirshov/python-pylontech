import json
import os

import requests

from pylontechpoller.reporter import Reporter, logger


class HassReporter(Reporter):
    def __init__(self, hass_url, hass_stack_disbalance, hass_max_battery_disbalance, hass_max_battery_disbalance_id, hass_token):
        self.hass_url = hass_url
        self.hass_stack_disbalance = hass_stack_disbalance
        self.hass_max_battery_disbalance = hass_max_battery_disbalance
        self.hass_max_battery_disbalance_id = hass_max_battery_disbalance_id
        if os.path.exists(hass_token):
            with open(hass_token, 'r') as file:
                hass_token = file.read().strip()
        self.hass_token = hass_token


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
