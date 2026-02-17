#!/bin/bash
# RustyGate - Start Remote Responder (Persistent)
# Usage: ./scripts/start_responder.sh [remote_interface]

if [ -f "test_config.sh" ]; then
    source test_config.sh
else
    echo "CRITICAL: test_config.sh not found."
    exit 1
fi

REMOTE_IFACE=${1:-wlan0}
REMOTE_TARGET="$REMOTE_USER@$REMOTE_IP"
SSH_OPTS="-i $SSH_KEY -o ConnectTimeout=5 -o BatchMode=yes"

echo "[$(date +%T)] Starting persistent responder on $REMOTE_IP ($REMOTE_IFACE)..."

# Sync latest code and build
ssh $SSH_OPTS "$REMOTE_TARGET" "cd $REMOTE_DIR && git pull origin main && cd tests/bacnet-responder && cargo build --quiet"

# Get local IP for responder to ping back
LOCAL_IP=$(ip route get $REMOTE_IP | grep -oP 'src \K\S+')
echo "Local Gateway IP detected: $LOCAL_IP"

# Kill existing and start new in background
ssh $SSH_OPTS "$REMOTE_TARGET" "pkill -9 -f bacnet-responder || true"
ssh $SSH_OPTS "$REMOTE_TARGET" "nohup sh -c 'cd $REMOTE_DIR/tests/bacnet-responder && ./target/debug/bacnet-responder $DEVICE_ID localhost $REMOTE_IFACE $LOCAL_IP' > $REMOTE_DIR/tests/bacnet-responder/responder.log 2>&1 < /dev/null &"

# Verify it started
sleep 2
if ssh $SSH_OPTS "$REMOTE_TARGET" "pgrep -f bacnet-responder" > /dev/null; then
    echo "OK: Responder is running."
else
    echo "ERROR: Responder failed to start."
    exit 1
fi
