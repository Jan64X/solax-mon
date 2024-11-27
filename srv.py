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
import sys

# Configure comprehensive logging
logging.basicConfig(
    level=logging.DEBUG,  # Changed to DEBUG for more detailed logging
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(sys.stdout),  # Log to console
        logging.FileHandler('solar_monitor.log')  # Log to file for persistent records
    ]
)
logger = logging.getLogger(__name__)

# Load configuration from secrets file
def load_secrets(secrets_file='secrets.txt'):
    """
    Load configuration parameters from a secrets file with comprehensive error handling.
    
    Args:
        secrets_file (str): Path to the secrets file. Defaults to 'secrets.txt'.
    
    Returns:
        dict: A dictionary of configuration parameters.
    
    Raises:
        ValueError: If any required configuration parameter is missing or invalid.
    """
    secrets = {}
    required_keys = [
        'DISCORD_WEBHOOK_URL', 
        'SERVER_IP', 
        'INVERTER_IP', 
        'INVERTER_PORT', 
        'INVERTER_SERIAL'
    ]

    # Log the current working directory and file path
    logger.info(f"Current working directory: {os.getcwd()}")
    logger.info(f"Attempting to read secrets file: {os.path.abspath(secrets_file)}")

    if not os.path.exists(secrets_file):
        logger.error(f"Secrets file {secrets_file} does not exist!")
        raise ValueError(f"Secrets file {secrets_file} not found!")

    try:
        with open(secrets_file, 'r') as f:
            for line in f:
                line = line.strip()
                if line and '=' in line:
                    key, value = line.split('=', 1)
                    # Remove quotes if present
                    secrets[key.strip()] = value.strip().strip('"\'')
                    logger.debug(f"Loaded key: {key.strip()}")
    except Exception as e:
        logger.error(f"Error reading secrets file: {e}")
        raise ValueError(f"Error reading secrets file: {e}")
    
    # Check for all required keys
    missing_keys = [key for key in required_keys if key not in secrets]
    if missing_keys:
        logger.error(f"Missing required configuration parameters: {', '.join(missing_keys)}")
        raise ValueError(f"Missing required configuration parameters: {', '.join(missing_keys)}")
    
    return secrets

# Load secrets
CONFIG = load_secrets()

# Explicitly extract configuration
DISCORD_WEBHOOK_URL = CONFIG['DISCORD_WEBHOOK_URL']
SERVER_IP = CONFIG['SERVER_IP']
INVERTER_IP = CONFIG['INVERTER_IP']
INVERTER_PORT = int(CONFIG['INVERTER_PORT'])
INVERTER_SERIAL = CONFIG['INVERTER_SERIAL']

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
                INVERTER_PORT, 
                INVERTER_SERIAL, 
                inverters=[INVERTERS_ENTRY_POINTS.get("x3_hybrid_g4")], 
                return_when=asyncio.FIRST_COMPLETED
            )
            return await inverter.get_data()
        except Exception as e:
            logger.error(f"Error fetching inverter data: {e}")
            return None

    def get_grid_status(self, grid_power):
        """
        Determine grid status. Grid power can be:
        - 0: Grid is truly disconnected
        - Negative: Consuming from grid
        - Positive: Feeding into grid
        """
        # Debug log the actual grid power value
        logger.debug(f"Raw grid power value: {grid_power} (type: {type(grid_power)})")
        
        try:
            grid_power = float(grid_power)  # Ensure we're working with a float
            if grid_power == 0.0:
                return "Disconnected", "OFF"
            elif grid_power < 0.0:
                return f"Consuming {abs(grid_power)}W", "ON"
            else:
                return f"Feeding {grid_power}W", "ON"
        except (TypeError, ValueError) as e:
            logger.error(f"Error processing grid power value: {e}")
            return "Error", "Unknown"

    def print_status(self, data):
        """Print current status to console"""
        try:
            # Get raw values
            grid_power = data.data.get('Grid Power ', 0.0)
            solar_power = data.data.get('PV1 Power', 0.0) + data.data.get('PV2 Power', 0.0)
            battery_percent = data.data.get('Battery Remaining Capacity', 0.0)
            home_consumption = data.data.get('Load/Generator Power', 0.0)
            
            # Debug print raw values
            logger.debug(f"Raw data values: Grid={grid_power}, Solar={solar_power}, Battery={battery_percent}, Home={home_consumption}")
            
            grid_status, grid_connection = self.get_grid_status(grid_power)
            
            status = f"""
[{datetime.now().strftime('%Y-%m-%d %H:%M:%S')}] System Status:
└─ Grid Power: {grid_power}W -> {grid_status} ({grid_connection})
└─ Solar Production: {solar_power}W
└─ Battery Level: {battery_percent}%
└─ Home Consumption: {home_consumption}W
"""
            print(status)
        except Exception as e:
            logger.error(f"Error in print_status: {e}")

    def check_critical_conditions(self, data):
        """Check if critical shutdown conditions are met"""
        try:
            solar_power = float(data.data.get('PV1 Power', 0.0)) + float(data.data.get('PV2 Power', 0.0))
            battery_percent = float(data.data.get('Battery Remaining Capacity', 0.0))
            grid_power = float(data.data.get('Grid Power ', 0.0))

            # Debug log the values being checked
            logger.debug(f"Critical check values: Solar={solar_power}W, Battery={battery_percent}%, Grid={grid_power}W")

            return (
                solar_power < 100.0 and 
                battery_percent < 5.0 and 
                grid_power == 0.0  # True grid disconnection
            )
        except (TypeError, ValueError) as e:
            logger.error(f"Error in critical conditions check: {e}")
            return False

    async def send_discord_alert(self, message):
        """Send alert to Discord webhook"""
        try:
            payload = {"content": message}
            response = requests.post(DISCORD_WEBHOOK_URL, json=payload)
            logger.info(f"Discord alert sent: {message}")
        except Exception as e:
            logger.error(f"Failed to send Discord alert: {e}")

    def shutdown_server(self):
        """Shutdown remote server via SSH - commented out for safety"""
        try:
            # Uncomment and configure when ready to actually implement
            # ssh = paramiko.SSHClient()
            # ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
            # ssh.connect(SERVER_IP, username='root')
            # ssh.exec_command('shutdown now')
            # ssh.close()
            logger.info("SHUTDOWN WOULD HAVE BEEN EXECUTED")
            pass
        except Exception as e:
            logger.error(f"SSH Shutdown failed: {e}")

    async def monitor_system(self):
        """Main monitoring coroutine"""
        while True:
            try:
                data = await self.get_inverter_data()
                if data:
                    self.current_data = data
                    self.print_status(data)
                    
                    if self.check_critical_conditions(data):
                        grid_power = float(data.data.get('Grid Power ', 0.0))
                        grid_status, grid_connection = self.get_grid_status(grid_power)
                        await self.send_discord_alert(
                            "⚠ CRITICAL SYSTEM ALERT: Potential Power Failure Imminent!\n"
                            f"Grid: {grid_status}\n"
                            f"Solar: {data.data.get('PV1 Power', 0.0) + data.data.get('PV2 Power', 0.0)}W\n"
                            f"Battery: {data.data.get('Battery Remaining Capacity', 0.0)}%"
                        )
                        # Wait a minute before potential shutdown
                        await asyncio.sleep(60)
                        self.shutdown_server()
            except Exception as e:
                logger.error(f"Monitoring error: {e}")
            
            await asyncio.sleep(60)

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
    logger.info("Web server thread started")

    # Start monitoring
    await monitor.monitor_system()

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as e:
        logger.error(f"Unhandled exception: {e}", exc_info=True)
        sys.exit(1)
