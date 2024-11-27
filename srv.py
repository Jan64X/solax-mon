import asyncio
import logging
import requests
import json
import solax
from importlib.metadata import entry_points
from http.server import BaseHTTPRequestHandler, HTTPServer
import threading
import paramiko
from datetime import datetime
import os

# Configure logging
logging.basicConfig(level=logging.INFO, 
                    format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

# Function to read the configuration from secrets.txt
def get_config():
    secrets_file = os.path.join(os.path.dirname(__file__), "secrets.txt")
    config = {}
    with open(secrets_file, "r") as file:
        for line in file:
            key, _, value = line.strip().partition("=")
            if key and value:
                config[key.strip()] = value.strip('"')
    required_keys = ["DISCORD_WEBHOOK_URL", "SERVER_IP", "INVERTER_IP", "INVERTER_SERIAL"]
    for key in required_keys:
        if key not in config:
            raise ValueError(f"Missing required configuration key: {key}")
    return config

# Load configuration
config = get_config()
DISCORD_WEBHOOK_URL = config["DISCORD_WEBHOOK_URL"]
SERVER_IP = config["SERVER_IP"]
INVERTER_IP = config["INVERTER_IP"]
INVERTER_SERIAL = config["INVERTER_SERIAL"]

class SolarMonitor:
    def __init__(self):
        self.last_alert_time = 0
        self.current_data = None

    async def get_inverter_data(self):
        """Fetch data from the solar inverter"""
        try:
            INVERTERS_ENTRY_POINTS = {
                ep.name: ep.load() for ep in entry_points(group="solax.inverter")
            }
            inverter = await solax.discover(
                INVERTER_IP, 
                80, 
                INVERTER_SERIAL, 
                inverters=[INVERTERS_ENTRY_POINTS.get("x3_hybrid_g4")], 
                return_when=asyncio.FIRST_COMPLETED
            )
            return await inverter.get_data()
        except Exception as e:
            logger.error(f"Error fetching inverter data: {e}")
            return None

    # Other methods remain unchanged...

class SolarStatusServer(BaseHTTPRequestHandler):
    def do_GET(self):
        if not hasattr(self.server, 'solar_monitor'):
            self.send_error(500, "Monitor not initialized")
            return

        monitor = self.server.solar_monitor
        data = monitor.current_data

        if not data:
            self.send_error(500, "No data available")
            return

        try:
            grid_power = float(data.data.get('Grid Power ', 0.0))
            grid_status, grid_connection = monitor.get_grid_status(grid_power)
            
            status = {
                "Grid": f"{grid_status} ({grid_connection})",
                "Solar Panels": f"{data.data.get('PV1 Power', 0.0) + data.data.get('PV2 Power', 0.0)}W",
                "Batteries": f"{data.data.get('Battery Remaining Capacity', 0.0)}%",
                "Home Consumption": f"{data.data.get('Load/Generator Power', 0.0)}W"
            }

            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(status).encode())
        except Exception as e:
            logger.error(f"Error in web server: {e}")
            self.send_error(500, str(e))

def run_web_server(monitor):
    """Run the web status server"""
    server_address = ('', 8000)
    httpd = HTTPServer(server_address, SolarStatusServer)
    httpd.solar_monitor = monitor
    logger.info("Web server running on port 8000")
    httpd.serve_forever()

async def main():
    monitor = SolarMonitor()
    
    # Start web server in a separate thread
    web_thread = threading.Thread(target=run_web_server, args=(monitor,), daemon=True)
    web_thread.start()

    # Start monitoring
    await monitor.monitor_system()

if __name__ == "__main__":
    asyncio.run(main())
