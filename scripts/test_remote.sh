#!/bin/bash
# RustyGate Remote Integration Test - ROBUST MODE
# Run from project root

# Load private configuration
if [ -f "test_config.sh" ]; then
    source test_config.sh
else
    echo "Error: test_config.sh not found."
    exit 1
fi

REMOTE_TARGET="$REMOTE_USER@$REMOTE_IP"

echo "[$(date +%T)] --- 1. Updating Code on Remote Machine ---"
ssh -i $SSH_KEY $REMOTE_TARGET "cd $REMOTE_DIR && git pull origin main"

echo "[$(date +%T)] --- 2. Starting Remote Responder ---"
ssh -i $SSH_KEY $REMOTE_TARGET "pkill -f bacnet-responder || true"

echo "[$(date +%T)] Building responder on remote..."
# Build first so we don't time out the background start
ssh -i $SSH_KEY $REMOTE_TARGET "cd $REMOTE_DIR/tests/bacnet-responder && cargo build"

echo "[$(date +%T)] Starting binary in background..."
# Use </dev/null to ensure SSH doesn't wait for the background process to exit
ssh -i $SSH_KEY $REMOTE_TARGET "cd $REMOTE_DIR/tests/bacnet-responder && \
    nohup ./target/debug/bacnet-responder $DEVICE_ID localhost $REMOTE_IFACE </dev/null >responder.log 2>&1 &"

echo "Waiting 10s for responder to bind port 47808..."
sleep 10

echo "[$(date +%T)] Checking remote logs:"
ssh -i $SSH_KEY $REMOTE_TARGET "tail -n 5 $REMOTE_DIR/tests/bacnet-responder/responder.log"

echo "[$(date +%T)] --- 3. Selecting Interfaces ---"
if [ -z "$1" ]; then
    echo "Error: No local interface specified."
    echo "Usage: ./scripts/test_remote.sh <local_interface> [remote_interface]"
    exit 1
fi
LOCAL_IFACE=$1
REMOTE_IFACE=${2:-wlan0}
echo "Local interface selected: $LOCAL_IFACE"
echo "Remote interface selected: $REMOTE_IFACE"

echo "[$(date +%T)] --- 4. Running Local Discovery ---"
RUST_LOG=info cargo run -- discover $LOCAL_IFACE | tee local_test.log

echo "[$(date +%T)] --- 5. Verifying Results ---"
if grep -q "ID=$DEVICE_ID" local_test.log; then
    echo "========================================"
    echo "SUCCESS: Remote device $DEVICE_ID discovered!"
    echo "========================================"
    RESULT=0
else
    echo "========================================"
    echo "FAILURE: Remote device $DEVICE_ID NOT found."
    echo "========================================"
    RESULT=1
fi

echo "[$(date +%T)] --- 6. Cleanup Remote Machine ---"
ssh -i $SSH_KEY $REMOTE_TARGET "pkill -f bacnet-responder"

exit $RESULT
