# WebSocket Troubleshooting Guide

This guide helps you diagnose issues with the Lighter WebSocket connection when no data updates are appearing in the logs.

## Log File Location

All debug logs are written to: `/tmp/hype_debug.log`

To monitor logs in real-time:
```bash
tail -f /tmp/hype_debug.log
```

## What to Look For in Logs

### 1. Connection Establishment

You should see:
```
[HH:MM:SS] lighter_websocket starting, exchange=2
[HH:MM:SS] Fetching Lighter market mapping...
[HH:MM:SS] Market map created with X entries
[HH:MM:SS] Connection attempt #1
[HH:MM:SS] Connecting to Lighter WebSocket: wss://...
[HH:MM:SS] Connected to Lighter WebSocket
[HH:MM:SS] Successfully sent subscription to Lighter WebSocket
[HH:MM:SS] Listening for Lighter messages with health check enabled...
```

**Problem**: If you don't see "Connected to Lighter WebSocket":
- Check your internet connection
- Check if the Lighter API is accessible: `curl https://mainnet.zklighter.elliot.ai/api/v1/funding-rates`
- Look for connection errors in the logs

### 2. Regular Ping Activity

Every 30 seconds you should see:
```
[HH:MM:SS] ⏰ PING: Sending ping to keep connection alive
[HH:MM:SS] ✓ Ping sent successfully
```

**Problem**: If pings stop or show errors:
- Connection has died
- Check for "Failed to send ping" errors
- Check for automatic reconnection attempts

### 3. Message Reception

When data is received, you should see:
```
[HH:MM:SS] Received text message: XXXX bytes
[HH:MM:SS] Raw message preview: {"channel":"market_stats/all",...}
[HH:MM:SS] Successfully parsed Lighter message with X market stats
[HH:MM:SS] Sent LT data: BTC-PERP exchange=2
```

**Problem**: If you see "Received text message" but no "Successfully parsed":
- The message format may have changed
- Check the raw message preview in the logs
- Compare with the expected MarketStatsMessage structure

### 4. Timeout Issues

If you see:
```
[HH:MM:SS] TIMEOUT: No message received within 60 seconds, reconnecting...
```

This means:
- The server is not sending any messages (not even pings)
- The subscription may not be working
- The connection is established but silent

## Common Issues and Solutions

### Issue 1: No Messages After Connection

**Symptoms**:
- Connection succeeds
- Subscription sent successfully
- Pings work
- But no market data messages

**Possible Causes**:
1. **Lighter API is not publishing updates**: The Lighter exchange may not be actively updating market stats
2. **Subscription channel changed**: The API may have changed the channel name from `market_stats/all`
3. **Message format changed**: The JSON structure may have changed

**Solutions**:
1. Test the example directly:
   ```bash
   cargo run --example WsLighter
   ```
   If the example works but the main app doesn't, compare the implementations.

2. Check if Lighter API is active:
   ```bash
   curl https://mainnet.zklighter.elliot.ai/api/v1/funding-rates
   ```
   This should return a list of markets. If empty or error, the API may be down.

3. Look for subscription confirmation in raw messages:
   - Check logs for any message that might be a subscription confirmation
   - The server might send an acknowledgment that we're not recognizing

### Issue 2: Frequent Reconnections

**Symptoms**:
```
[HH:MM:SS] TIMEOUT: No message received within 60 seconds, reconnecting...
[HH:MM:SS] Reconnecting in Xs...
[HH:MM:SS] Connection attempt #X
```

**Possible Causes**:
1. Server is not sending any messages
2. Server dropped the connection
3. Network issues

**Solutions**:
1. Increase timeout duration (edit `Duration::from_secs(60)` to a higher value)
2. Check if the Lighter WebSocket URL has changed
3. Verify the subscription message format with Lighter documentation

### Issue 3: Connection Immediately Fails

**Symptoms**:
```
[HH:MM:SS] Lighter connection failed: ..., retrying in Xs
```

**Solutions**:
1. Check DNS resolution: `nslookup mainnet.zklighter.elliot.ai`
2. Check TLS/SSL certificates
3. Try connecting with a WebSocket client tool:
   ```bash
   websocat wss://mainnet.zklighter.elliot.ai/stream
   ```

### Issue 4: Parse Errors

**Symptoms**:
```
[HH:MM:SS] Failed to parse message as MarketStatsMessage. First 300 chars: ...
```

**Solutions**:
1. Copy the raw message from logs
2. Compare with the MarketStatsMessage struct in `src/third_party/lighter/data.rs`
3. Update the struct if the API format has changed

## Advanced Debugging

### Enable More Verbose Logging

Edit `src/websocket/client.rs` and modify the log_debug function to also print to stderr:
```rust
fn log_debug(msg: String) {
    eprintln!("[DEBUG] {}", msg); // Add this line
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/hype_debug.log")
    {
        // ... existing code
    }
}
```

### Test with WebSocket Client

Use `websocat` or similar to test the connection manually:
```bash
# Install websocat
brew install websocat  # macOS
# or
cargo install websocat

# Connect to Lighter
websocat wss://mainnet.zklighter.elliot.ai/stream

# Then send subscription message:
{"type":"subscribe","channel":"market_stats/all"}
```

### Capture Network Traffic

Use Wireshark or `tcpdump` to capture WebSocket traffic:
```bash
sudo tcpdump -i any -s 0 -w lighter_ws.pcap host mainnet.zklighter.elliot.ai
```

Then open `lighter_ws.pcap` in Wireshark and filter for WebSocket frames.

## Checking If It's a Lighter API Issue

1. **Check their status page** (if they have one)
2. **Check if REST API works**:
   ```bash
   curl https://mainnet.zklighter.elliot.ai/api/v1/funding-rates
   ```
3. **Compare with other exchanges**: Switch to Hyperliquid (exchange=1) and see if that works

## Expected Behavior

When everything is working correctly, you should see:

1. Connection established within 1-2 seconds
2. Subscription sent successfully
3. **First market stats message** within a few seconds
4. **Regular ping messages** every 30 seconds
5. **Market stats updates** - frequency depends on Lighter's update rate (could be every few seconds to every few minutes)
6. No timeout or reconnection messages

## Still Not Working?

If you've tried everything above and still have issues:

1. **Verify the subscription is correct** by testing with the standalone example
2. **Check Lighter's documentation** for any API changes
3. **Contact Lighter support** to verify the WebSocket endpoint is active
4. **Consider using HTTP polling** as a fallback if WebSocket is unreliable

## Quick Test Command

Run this to see if the basic WebSocket connection works:
```bash
cd hype
cargo run --example WsLighter
```

If the example works but the main app doesn't, the issue is in how the app integrates the WebSocket, not with the Lighter API itself.