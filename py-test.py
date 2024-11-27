from importlib.metadata import entry_points
import solax
import asyncio

INVERTERS_ENTRY_POINTS = {
   ep.name: ep.load() for ep in entry_points(group="solax.inverter")
}

async def work():
    inverter = await solax.discover("5.8.8.8", 80, "SERIALHERE", inverters=[INVERTERS_ENTRY_POINTS.get("x3_hybrid_g4")], return_when=asyncio.FIRST_COMPLETED)
    return await inverter.get_data()

loop = asyncio.new_event_loop()
asyncio.set_event_loop(loop)
data = loop.run_until_complete(work())
print(data)
