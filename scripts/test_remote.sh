#!/bin/bash
# RustyGate Remote Integration Test
# Run from project root

# Load private configuration
if [ -f "test_config.sh" ]; then
    source test_config.sh
else
    echo "Error: test_config.sh not found. Please copy test_config.example to test_config.sh and fill in details."
    exit 1
fi

REMOTE_TARGET="$REMOTE_USER@$REMOTE_IP"

echo "--- 1. Updating Code on Remote Machine ---"
ssh -i $SSH_KEY $REMOTE_TARGET "cd $REMOTE_DIR && git pull origin main"

echo "--- 2. Starting Remote Responder (Background) ---"
# Kill any existing responder first
ssh -i $SSH_KEY $REMOTE_TARGET "pkill -f bacnet-responder || true"
# Build and run the responder from its own directory
ssh -i $SSH_KEY $REMOTE_TARGET "cd $REMOTE_DIR/tests/bacnet-responder && cargo build && nohup cargo run -- $DEVICE_ID > responder.log 2>&1 &"

# Give it a moment to start and bind port
echo "Waiting for remote responder to stabilize..."
sleep 10

echo "--- 3. Detecting Local Interface ---"
LOCAL_IFACE=$(ip route get $REMOTE_IP | grep -oP 'dev \K\S+')
echo "Local interface detected: $LOCAL_IFACE"

echo "--- 4. Running Local Discovery ---"
# Run local gateway in discovery mode
OUTPUT=$(cargo run --quiet -- discover $LOCAL_IFACE)
echo "$OUTPUT"

echo "--- 5. Verifying Results ---"
if echo "$OUTPUT" | grep -q "ID=$DEVICE_ID"; then
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

echo "--- 6. Cleanup Remote Machine ---"
ssh -i $SSH_KEY $REMOTE_TARGET "pkill -f bacnet-responder"

exit $RESULT
