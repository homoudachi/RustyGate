#!/bin/bash
# RustyGate Object Discovery Integration Test
# Usage: ./scripts/test_objects.sh <local_interface> [remote_interface]

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

echo "[$(date +%T)] === Starting Object Discovery Test ==="
echo "Local Interface:  $LOCAL_IFACE"
echo "Remote Target:    $REMOTE_TARGET"
echo "Remote Interface: $REMOTE_IFACE"

# 1. Start Responder
echo "[$(date +%T)] Syncing and restarting remote responder..."
ssh $SSH_OPTS "$REMOTE_TARGET" "cd $REMOTE_DIR && git pull origin main && cd tests/bacnet-responder && cargo build --quiet"
ssh $SSH_OPTS "$REMOTE_TARGET" "pkill -9 -f bacnet-responder || true"
ssh $SSH_OPTS "$REMOTE_TARGET" "nohup sh -c 'cd $REMOTE_DIR/tests/bacnet-responder && ./target/debug/bacnet-responder $DEVICE_ID localhost $REMOTE_IFACE' > $REMOTE_DIR/tests/bacnet-responder/responder.log 2>&1 < /dev/null &"
sleep 3

# 2. Verify Connectivity (Ping)
echo "[$(date +%T)] Verifying connectivity via Ping..."
RUST_LOG=info cargo run -- ping "$LOCAL_IFACE" "$REMOTE_IP" > ping_result.log 2>&1
if grep -q "FOUND DEVICE: ID=$DEVICE_ID" ping_result.log; then
    echo "OK: Responder is reachable."
else
    echo "ERROR: Responder not reachable via ping."
    cat ping_result.log
    exit 1
fi

# 3. Discover Objects
echo "[$(date +%T)] Running object discovery..."
# The responder is on REMOTE_IP:47808
RUST_LOG=info cargo run -- discover-objects "$LOCAL_IFACE" "$DEVICE_ID" "$REMOTE_IP:47808" | tee discovery_objects.log

# 4. Verification
echo "[$(date +%T)] Verifying discovered objects..."
if grep -q "AnalogInput" discovery_objects.log && grep -q "BinaryInput" discovery_objects.log; then
    echo "========================================"
    echo "SUCCESS: Objects discovered successfully!"
    echo "========================================"
    RESULT=0
else
    echo "========================================"
    echo "FAILURE: Could not discover all expected objects."
    echo "DIAGNOSTIC: Remote logs:"
    ssh $SSH_OPTS "$REMOTE_TARGET" "tail -n 20 $REMOTE_DIR/tests/bacnet-responder/responder.log"
    echo "========================================"
    RESULT=1
fi

# 5. Cleanup
echo "[$(date +%T)] Cleaning up..."
ssh $SSH_OPTS "$REMOTE_TARGET" "pkill -f bacnet-responder"

exit $RESULT
