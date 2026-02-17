#!/bin/bash
# RustyGate - UI Backend API Automation Test (ROBUST)
# Usage: ./scripts/test_ui_api.sh <local_interface> [remote_interface]

if [ -z "$1" ]; then
    echo "Usage: $0 <local_interface> [remote_interface]"
    exit 1
fi

LOCAL_IFACE=$1
REMOTE_IFACE=${2:-wlan0}
API="http://localhost:8080/api"

# Load private configuration
if [ -f "test_config.sh" ]; then
    source test_config.sh
else
    echo "CRITICAL: test_config.sh not found."
    exit 1
fi

echo "[$(date +%T)] === Starting Robust UI Backend Test ==="

# Cleanup function
cleanup() {
    echo "[$(date +%T)] Cleaning up processes..."
    [ -n "$GATEWAY_PID" ] && kill $GATEWAY_PID 2>/dev/null
    ssh -i $SSH_KEY $REMOTE_USER@$REMOTE_IP "pkill -9 -f bacnet-responder || true"
    echo "Cleanup complete."
}

# Trap exit/errors to ensure cleanup
trap cleanup EXIT

# 1. Start Remote Responder
echo "Starting remote responder on $REMOTE_IP ($REMOTE_IFACE)..."
./scripts/start_responder.sh $REMOTE_IFACE
if [ $? -ne 0 ]; then
    echo "ERROR: Failed to start responder."
    exit 1
fi

# 2. Start Gateway in background
echo "Starting gateway locally..."
cargo build --quiet
RUST_LOG=info cargo run > gateway_ui_test.log 2>&1 &
GATEWAY_PID=$!

# Wait for gateway to start
sleep 3

# 3. Check Interfaces
echo "Listing interfaces..."
curl -s $API/interfaces | grep -q "$LOCAL_IFACE"
if [ $? -eq 0 ]; then
    echo "OK: Interface $LOCAL_IFACE found."
else
    echo "ERROR: Interface $LOCAL_IFACE not found in API list."
    exit 1
fi

# 4. Bind Interface
echo "Binding to $LOCAL_IFACE..."
curl -s -X POST -H "Content-Type: application/json" -d "{\"interface_name\": \"$LOCAL_IFACE\"}" "$API/bind"
sleep 5 # Increase wait time for binding and thread start

# 5. Trigger Discovery (Try Ping first as it is more reliable on some networks)
echo "Pinging responder at $REMOTE_IP..."
curl -s -X POST -H "Content-Type: application/json" -d "{\"target_ip\": \"$REMOTE_IP\"}" "$API/ping"
sleep 5

echo "Triggering general discovery scan..."
curl -s -X POST $API/discover
echo "Waiting 15s for discovery results..."
sleep 15

# 6. Check Discovered Devices
echo "Checking discovered devices..."
DEVICES=$(curl -s $API/devices)
echo "Devices found by API: $DEVICES"

# Basic Diagnostics
echo "--- Remote Responder Diagnostics ---"
ssh -i $SSH_KEY $REMOTE_USER@$REMOTE_IP "tail -n 10 $REMOTE_DIR/tests/bacnet-responder/responder.log"

if [[ "$DEVICES" == *"instance"* ]]; then
    DEVICE_ID=$(echo $DEVICES | grep -oP '"instance":\K\d+' | head -n 1)
    echo "SUCCESS: Found device $DEVICE_ID"
    
    # 7. Check Objects
    echo "Requesting objects for device $DEVICE_ID..."
    # The first call to /objects triggers discovery if not already discovered
    curl -s $API/devices/$DEVICE_ID/objects > /dev/null
    echo "Waiting 5s for object enumeration..."
    sleep 5
    
    OBJECTS=$(curl -s $API/devices/$DEVICE_ID/objects)
    echo "Objects found: $(echo $OBJECTS | grep -o "object_type" | wc -l)"
    
    if [[ "$OBJECTS" == *"AnalogInput"* ]]; then
        echo "========================================"
        echo "SUCCESS: UI API Discovery verified!"
        echo "========================================"
        RESULT=0
    else
        echo "FAILURE: Device found but objects NOT discovered."
        RESULT=1
    fi
else
    echo "FAILURE: No devices found through API."
    echo "--- Gateway Logs ---"
    tail -n 30 gateway_ui_test.log
    RESULT=1
fi

exit $RESULT
