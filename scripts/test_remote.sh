#!/bin/bash
# RustyGate Smarter Remote Integration Test
# Usage: ./scripts/test_remote.sh <local_interface> [remote_interface]

# Load private configuration
if [ -f "test_config.sh" ]; then
    source test_config.sh
else
    echo "CRITICAL: test_config.sh not found."
    exit 1
fi

LOCAL_IFACE=$1
REMOTE_IFACE=${2:-wlan0}
REMOTE_TARGET="$REMOTE_USER@$REMOTE_IP"
SSH_OPTS="-i $SSH_KEY -o ConnectTimeout=5 -o BatchMode=yes"

if [ -z "$LOCAL_IFACE" ]; then
    echo "Usage: $0 <local_interface> [remote_interface]"
    exit 1
fi

echo "[$(date +%T)] === Starting Smart Test ==="
echo "Local Interface:  $LOCAL_IFACE"
echo "Remote Target:    $REMOTE_TARGET"
echo "Remote Interface: $REMOTE_IFACE"

# 1. Connectivity Check
echo -n "[$(date +%T)] Checking SSH connectivity... "
if ssh $SSH_OPTS "$REMOTE_TARGET" "echo 'up'" >/dev/null 2>&1; then
    echo "OK"
else
    echo "FAILED"
    echo "Error: Cannot reach remote host $REMOTE_IP via SSH."
    exit 1
fi

# 2. Remote Sync & Build
echo "[$(date +%T)] Syncing and building on remote..."
ssh $SSH_OPTS "$REMOTE_TARGET" "cd $REMOTE_DIR && git pull origin main && cd tests/bacnet-responder && cargo build --quiet"
if [ $? -ne 0 ]; then
    echo "ERROR: Remote build failed."
    exit 1
fi

# 3. Start Responder
echo "[$(date +%T)] Killing old responder..."
ssh $SSH_OPTS "$REMOTE_TARGET" "pkill -9 -f bacnet-responder || true"
sleep 2

echo "[$(date +%T)] Starting remote responder (robustly)..."
# Use nohup, double fork, and redirect all streams including stdin
ssh $SSH_OPTS "$REMOTE_TARGET" "nohup sh -c 'cd $REMOTE_DIR/tests/bacnet-responder && ./target/debug/bacnet-responder $DEVICE_ID localhost $REMOTE_IFACE' > $REMOTE_DIR/tests/bacnet-responder/responder.log 2>&1 < /dev/null &"

echo "[$(date +%T)] Triggered background process. Checking if it's running..."
sleep 2
ssh $SSH_OPTS "$REMOTE_TARGET" "pgrep -f bacnet-responder > /dev/null && echo 'Process is alive' || echo 'Process NOT found'"
# 4. Verification of Responder
echo -n "[$(date +%T)] Waiting for responder to bind... "
MAX_RETRIES=10
SUCCESS=0
for i in $(seq 1 $MAX_RETRIES); do
    if ssh $SSH_OPTS "$REMOTE_TARGET" "ss -lnup | grep -q 47808" >/dev/null 2>&1; then
        echo "OK (Bound after ${i}s)"
        SUCCESS=1
        break
    fi
    echo -n "."
    sleep 1
done

if [ $SUCCESS -eq 0 ]; then
    echo "FAILED"
    echo "ERROR: Responder failed to bind to 47808 on remote."
    echo "--- Last 10 lines of remote log ---"
    ssh $SSH_OPTS "$REMOTE_TARGET" "tail -n 10 $REMOTE_DIR/tests/bacnet-responder/responder.log"
    exit 1
fi

# 5. Local Discovery
echo "[$(date +%T)] Running targeted discovery (ping) on $LOCAL_IFACE to $REMOTE_IP..."
# Start a local tcpdump in the background to see if we see any BACnet traffic
sudo tcpdump -i $LOCAL_IFACE -n udp port 47808 -c 10 > discovery_packets.log 2>&1 &
TCPDUMP_PID=$!

RUST_LOG=info cargo run -- ping "$LOCAL_IFACE" "$REMOTE_IP" | tee local_test.log

kill $TCPDUMP_PID 2>/dev/null || true

# 6. Result Verification
if grep -q "ID=$DEVICE_ID" local_test.log; then
    echo "========================================"
    echo "SUCCESS: Remote device $DEVICE_ID discovered!"
    echo "========================================"
    RESULT=0
else
    echo "========================================"
    echo "FAILURE: Remote device $DEVICE_ID NOT found."
    echo "----------------------------------------"
    echo "NETWORK DIAGNOSTIC (Local tcpdump):"
    cat discovery_packets.log
    echo "----------------------------------------"
    echo "DIAGNOSTIC: Remote logs for discovery window:"
    ssh $SSH_OPTS "$REMOTE_TARGET" "tail -n 20 $REMOTE_DIR/tests/bacnet-responder/responder.log"
    echo "========================================"
    RESULT=1
fi

# 7. Cleanup
echo "[$(date +%T)] Cleaning up remote responder..."
ssh $SSH_OPTS "$REMOTE_TARGET" "pkill -f bacnet-responder"

exit $RESULT
