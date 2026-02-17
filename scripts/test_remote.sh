#!/bin/bash
# RustyGate Remote Integration Test - VERBOSE TRIAGE MODE
# Run from project root

# Load private configuration
if [ -f "test_config.sh" ]; then
    source test_config.sh
else
    echo "Error: test_config.sh not found."
    exit 1
fi

REMOTE_TARGET="$REMOTE_USER@$REMOTE_IP"

echo "--- 1. Updating Code on Remote Machine ---"
ssh -i $SSH_KEY $REMOTE_TARGET "cd $REMOTE_DIR && git pull origin main"

echo "--- 2. Starting Remote Responder ---"
# Kill any existing responder first
ssh -i $SSH_KEY $REMOTE_TARGET "pkill -f bacnet-responder || true"

echo "Compiling and starting responder on remote (this may take a moment)..."
# Start responder and capture PID
# We use a subshell to nohup and disassociate
ssh -i $SSH_KEY $REMOTE_TARGET "cd $REMOTE_DIR/tests/bacnet-responder && \
    RUST_LOG=info cargo build && \
    nohup target/debug/bacnet-responder $DEVICE_ID > responder.log 2>&1 &"

# Triage remote startup
echo "Checking remote log for success..."
sleep 5
ssh -i $SSH_KEY $REMOTE_TARGET "tail -n 20 $REMOTE_DIR/tests/bacnet-responder/responder.log"

echo "--- 3. Detecting Local Interface ---"
LOCAL_IFACE=$(ip route get $REMOTE_IP | grep -oP 'dev \K\S+')
echo "Local interface detected: $LOCAL_IFACE"

echo "--- 4. Running Local Discovery (with RUST_LOG=info) ---"
# We run with RUST_LOG=info to see the core threads working
RUST_LOG=info cargo run -- discover $LOCAL_IFACE | tee local_test.log

echo "--- 5. Verifying Results ---"
if grep -q "ID=$DEVICE_ID" local_test.log; then
    echo "========================================"
    echo "SUCCESS: Remote device $DEVICE_ID discovered!"
    echo "========================================"
    RESULT=0
else
    echo "========================================"
    echo "FAILURE: Remote device $DEVICE_ID NOT found."
    echo "Check local_test.log and remote responder.log"
    echo "========================================"
    RESULT=1
fi

echo "--- 6. Cleanup Remote Machine ---"
ssh -i $SSH_KEY $REMOTE_TARGET "pkill -f bacnet-responder"

exit $RESULT
