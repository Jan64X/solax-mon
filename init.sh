#!/bin/sh

# Function to start solax-mon binary
start_solax_mon() {
    while true; do
        echo "Starting solax-mon..."
        /srv/solax-mon/solax-mon
        echo "solax-mon exited with status $?, restarting in 5 seconds..."
        sleep 5
    done
}

# Function to start ssh binary
start_ssh() {
    while true; do
        echo "Starting ssh..."
        /srv/solax-mon/ssh
        echo "ssh exited with status $?, restarting in 5 seconds..."
        sleep 5
    done
}

# Start solax-mon in background
start_solax_mon &

# Check if SSH should be enabled
if [ -f "/srv/solax-mon/data/secrets.txt" ]; then
    SSH_ENABLED=$(grep "SSH=true" "/srv/solax-mon/data/secrets.txt")
    if [ ! -z "$SSH_ENABLED" ]; then
        echo "SSH is enabled, starting ssh binary..."
        sleep 5 # arbitrary sleep to give solax-mon time to get first data
        start_ssh &
    else
        echo "SSH is not enabled in secrets.txt"
    fi
else
    echo "secrets.txt not found, SSH will not be started"
fi

# Keep the container running
wait