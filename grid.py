from importlib.metadata import entry_points
import solax
import asyncio
import os
import logging

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

def determine_grid_status(inverter_response):
    """
    Analyze inverter data to determine grid connection status.
    
    Args:
        inverter_response (InverterResponse): Inverter response object
    
    Returns:
        str: Grid status ('On-Grid', 'Off-Grid', or 'Uncertain')
    """
    try:
        # Access the data dictionary directly from the inverter response
        data = inverter_response.data

        # Check Run Mode
        run_mode = data.get('Run mode text', '').lower()
        if run_mode == 'normal':
            # Additional on-grid verification
            grid_power = data.get('Grid Power ', 0)
            grid_voltages = [
                data.get('Grid 1 Voltage', 0),
                data.get('Grid 2 Voltage', 0),
                data.get('Grid 3 Voltage', 0)
            ]
            
            # Criteria for on-grid:
            # 1. Run mode is 'Normal'
            # 2. Grid voltages are present (> 0)
            # 3. Grid power shows interaction (allows small variations)
            if any(voltage > 0 for voltage in grid_voltages) and abs(grid_power) < 500:
                return 'On-Grid'
        
        # Check EPS (Emergency Power Supply) for off-grid
        eps_power = [
            data.get('EPS 1 Power', 0),
            data.get('EPS 2 Power', 0),
            data.get('EPS 3 Power', 0)
        ]
        if any(power > 0 for power in eps_power):
            return 'Off-Grid'
        
        return 'Uncertain'
    
    except Exception as e:
        logging.error(f"Error determining grid status: {e}")
        return 'Uncertain'

async def get_inverter_data():
    """
    Retrieve inverter data using existing configuration.
    
    Returns:
        InverterResponse: Inverter data response
    """
    serial, ip = get_config()
    inverters = {ep.name: ep.load() for ep in entry_points(group="solax.inverter")}
    
    inverter = await solax.discover(
        ip, 80, serial,
        inverters=[inverters.get("x3_hybrid_g4")],
        return_when=asyncio.FIRST_COMPLETED
    )
    return await inverter.get_data()

async def main():
    """
    Main async function to retrieve and analyze inverter grid status.
    """
    try:
        data = await get_inverter_data()
        grid_status = determine_grid_status(data)
        print(f"Current Grid Status: {grid_status}")
        return grid_status
    except Exception as e:
        logging.error(f"Error in main process: {e}")
        return None

if __name__ == "__main__":
    # Configure logging
    logging.basicConfig(level=logging.INFO)
    
    # Run the async main function
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    status = loop.run_until_complete(main())
