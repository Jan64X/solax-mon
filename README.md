# solax-mon

Monitoring program in Rust for the Solax x3 hybrid gen4. Right now it only prints the most important formatted data.

TODO:

- Rewrite the code to fix warnings
- Do more testing

You can use the Dockerfile to build a docker container for you, and then use this to run it:
docker run -v /srv/solax-mon/data:/srv/solax-mon/data -p 3000:3000 solax-mon

User data should be stored in /srv/solax-mon/data
The secrets.txt file should contain attributes for your situation:

- INVERTER_IP=10.0.0.69
- SERIAL=someserial
- DISCORD_WEBHOOK=<https://discord.com/api/webhooks/...>
- SERVER=user@10.0.0.70
- HAVE_IDRAC=true/false
- IDRAC_SERVER=10.0.0.6,root,password
- SSH=true/false
