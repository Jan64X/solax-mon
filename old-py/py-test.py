from importlib.metadata import entry_points
import solax
import asyncio
import os

# Function to read the configuration from secrets.txt
def get_config():
    secrets_file = os.path.join(os.path.dirname(__file__), "secrets.txt")
    config = {}
    with open(secrets_file, "r") as file:
        for line in file:
            key, _, value = line.strip().partition("=")
            if key and value:
                config[key.strip()] = value.strip('"')
    if "INVERTER_SERIAL" not in config or "INVERTER_IP" not in config:
        raise ValueError("Both INVERTER_SERIAL and INVERTER_IP must be specified in secrets.txt")
    return config["INVERTER_SERIAL"], config["INVERTER_IP"]

# Load inverter entry points
INVERTERS_ENTRY_POINTS = {
    ep.name: ep.load() for ep in entry_points(group="solax.inverter")
}

async def work():
    serial, ip = get_config()
    inverter = await solax.discover(
        ip, 80, serial,
        inverters=[INVERTERS_ENTRY_POINTS.get("x3_hybrid_g4")],
        return_when=asyncio.FIRST_COMPLETED
    )
    return await inverter.get_data()

# Run the asyncio event loop
loop = asyncio.new_event_loop()
asyncio.set_event_loop(loop)
data = loop.run_until_complete(work())
print(data)
