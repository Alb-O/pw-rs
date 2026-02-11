#!/usr/bin/env bash
# Start pw daemon detached from terminal
# Usage: ./start-daemon.sh

# Check if already running
if pw daemon status &>/dev/null; then
    echo "Daemon already running"
    exit 0
fi

# Start daemon in background, detached from terminal
nohup pw daemon start --foreground &>/dev/null &
disown

# Wait for it to be ready
for i in {1..30}; do
    if pw daemon status &>/dev/null; then
        echo "Daemon started"
        exit 0
    fi
    sleep 0.1
done

echo "Failed to start daemon"
exit 1
