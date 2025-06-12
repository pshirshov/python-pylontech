import json


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
