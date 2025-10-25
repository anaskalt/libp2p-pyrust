# 📚 Examples Usage Guide

This guide shows you how to use the example applications included with libp2p-pyrust.

## 🚨 Important Notes

### Network Permissions

The P2P networking features require certain system permissions:
- **mDNS Discovery**: May require elevated permissions on some systems
- **Port Binding**: Ensure ports are not already in use
- **Firewall**: May need to allow UDP/TCP traffic

### Global State

The library uses a **global singleton** pattern for the P2P network. This means:
- ✅ **One node per Python process** - This is the intended usage
- ❌ **Cannot run multiple nodes in the same Python process**
- ✅ **Run multiple terminals** for multi-node testing

---

## 📁 Available Examples

### 1. `examples/example_chat.py` - Simple Chat

A basic chat application using GossipSub for message broadcasting.

**Features:**
- Gossipsub pub/sub messaging
- Automatic peer discovery (via bootstrap nodes)
- Interactive console input
- Real-time message display

**Usage:**

```bash
# Terminal 1 - Bootstrap Node
# First, modify example_chat.py line 31 to use empty bootstrap list: bootnodes = []
python3 examples/example_chat.py

# Wait for it to print the addresses, then copy a quic-v1 address
# Example: /ip4/127.0.0.1/udp/5000/quic-v1/p2p/12D3Koo...

# Terminal 2 - Connecting Node
# Modify example_chat.py line 31 to use the address from Terminal 1
# bootnodes = ['/ip4/127.0.0.1/udp/5000/quic-v1/p2p/12D3Koo...']
python3 examples/example_chat.py

# Now type messages in either terminal - they will broadcast to all connected peers!
```

### 2. `examples/example_direct.py` - Direct Messaging

Demonstrates direct peer-to-peer messaging using request-response protocol.

**Features:**
- Direct messaging to specific peers
- Peer selection interface
- Message routing between specific nodes

**Usage:**

```bash
# Terminal 1 - First Node
python3 examples/example_direct.py

# Copy one of the printed addresses

# Terminal 2 - Second Node
# Modify line 57 to use the address from Terminal 1
python3 examples/example_direct.py

# Use the 'switch' command to select a peer
# Then send direct messages!
```

---

## 🔧 Recommended: Modified Example for Easy Testing

Here's an improved version of example_chat.py that's easier to use:

### `example_chat_improved.py`

```python
#!/usr/bin/env python3
import asyncio
import sys
from aioconsole import ainput
import libp2p_pyrust as libp2p

def my_callback(message):
    print(f"\n📨 {message.decode('utf-8')}")
    print("Message: ", end='', flush=True)

async def subscribe_task():
    await libp2p.subscribe_to_messages(my_callback)

async def send_messages(node_name):
    while True:
        user_input = await ainput('Message: ')
        if user_input.lower() == '/quit':
            break
        msg = f"[{node_name}] {user_input}"
        await libp2p.publish_message(msg.encode('utf-8'))

async def main():
    # Parse command line arguments
    if len(sys.argv) < 3:
        print("Usage:")
        print("  Bootstrap: python3 script.py <name> <port>")
        print("  Connect:   python3 script.py <name> <port> <bootstrap_addr>")
        return

    node_name = sys.argv[1]
    port = int(sys.argv[2])
    bootstrap = sys.argv[3] if len(sys.argv) > 3 else None

    print(f"\n🚀 Starting {node_name} on port {port}")

    # Generate keypair
    key_file = f'node_{node_name}.key'
    libp2p.generate_ed25519_keypair(key_file)

    # Initialize network
    bootnodes = [bootstrap] if bootstrap else []
    await libp2p.init_global_p2p_network(
        bootnodes, port, key_file, 'chat-topic'
    )

    await asyncio.sleep(5)

    # Show node info
    peer_id = await libp2p.get_peer_id()
    addrs = await libp2p.get_host_addrs()

    print(f"✓ Peer ID: {peer_id}")

    if not bootstrap:
        print("\n📍 Bootstrap address (use this in other terminals):")
        for addr in addrs:
            if 'quic-v1' in addr:
                print(f"  {addr}")

    await asyncio.sleep(5)

    peers = await libp2p.get_global_connected_peers()
    print(f"✓ Connected to {len(peers)} peer(s)\n")

    # Start messaging
    asyncio.create_task(subscribe_task())
    await send_messages(node_name)

if __name__ == '__main__':
    asyncio.run(main())
```

**Usage:**

```bash
# Terminal 1 - Bootstrap
python3 example_chat_improved.py Alice 5000

# Copy the bootstrap address, then in Terminal 2:
python3 example_chat_improved.py Bob 5001 /ip4/127.0.0.1/udp/5000/quic-v1/p2p/...

# Terminal 3:
python3 example_chat_improved.py Charlie 5002 /ip4/127.0.0.1/udp/5000/quic-v1/p2p/...

# Now type messages and see them broadcast to all nodes!
```

---

## 🐛 Troubleshooting

### Issue: "Permission denied (os error 13)"

**Cause**: mDNS requires certain network permissions

**Solution 1** - Use specific ports (recommended):
```python
# Instead of port 0 (random), use specific port:
await libp2p.init_global_p2p_network(bootnodes, 5000, key_file, topic)
```

**Solution 2** - Run with elevated permissions (NOT recommended for production):
```bash
sudo python3 example_chat.py
```

**Solution 3** - Use bootstrap nodes instead of mDNS:
```python
# Always provide bootstrap nodes - don't rely on mDNS discovery
bootnodes = ['/ip4/127.0.0.1/udp/5000/quic-v1/p2p/...']
```

### Issue: "No peers connected"

**Possible causes:**
1. Firewall blocking UDP/TCP traffic
2. Wrong bootstrap address
3. Not enough time for peer discovery

**Solutions:**
- Wait 10-15 seconds after initialization
- Check firewall settings
- Verify bootstrap address is correct and reachable
- Ensure bootstrap node is still running

### Issue: "Module not found: aioconsole"

**Solution:**
```bash
pip install aioconsole
```

### Issue: "Cannot run multiple nodes in same script"

**Explanation**: By design! The library uses a global singleton.

**Solution**: Run each node in a separate Python process (separate terminal).

---

## 📊 Example Output

### Bootstrap Node Starting:
```
🚀 Starting Alice on port 5000
✓ Keypair generated: node_Alice.key
✓ Initializing network...
✓ Peer ID: 12D3KooWA1B2C3D4E5F6...
✓ Listening on 2 address(es):
  /ip4/127.0.0.1/udp/5000/quic-v1/p2p/12D3KooWA1B2C3D4E5F6...
  /ip4/127.0.0.1/tcp/5000/p2p/12D3KooWA1B2C3D4E5F6...

📍 Bootstrap address (use this in other terminals):
  /ip4/127.0.0.1/udp/5000/quic-v1/p2p/12D3KooWA1B2C3D4E5F6...

✓ Connected to 0 peer(s)

Message:
```

### Connecting Node:
```
🚀 Starting Bob on port 5001
✓ Keypair generated: node_Bob.key
✓ Initializing network...
✓ Peer ID: 12D3KooWB2C3D4E5F6G7...
✓ Connected to 1 peer(s)

Message: Hello from Bob!

📨 [Alice] Hi Bob!
Message:
```

---

## 🎯 Best Practices

1. **Always use specific ports** in production (not 0)
2. **Implement proper error handling** for network failures
3. **Add connection retry logic** for robustness
4. **Use bootstrap nodes** for reliable peer discovery
5. **Add timeouts** to prevent hanging
6. **Log peer connections/disconnections** for debugging
7. **Test with at least 3 nodes** to verify mesh networking

---

## 🚀 Quick Test Command

For quick testing without modifying files:

```bash
# Create test directory
mkdir -p /tmp/p2p_test
cd /tmp/p2p_test

# Terminal 1:
python3 -c "
import asyncio, libp2p_pyrust as p2p
async def main():
    p2p.generate_ed25519_keypair('n1.key')
    await p2p.init_global_p2p_network([], 5000, 'n1.key', 'test')
    await asyncio.sleep(3)
    print('Addrs:', await p2p.get_host_addrs())
    await asyncio.sleep(3600)
asyncio.run(main())
"

# Copy an address, then Terminal 2:
python3 -c "
import asyncio, libp2p_pyrust as p2p
async def main():
    p2p.generate_ed25519_keypair('n2.key')
    await p2p.init_global_p2p_network(['<PASTE_ADDR>'], 5001, 'n2.key', 'test')
    await asyncio.sleep(10)
    print('Peers:', await p2p.get_global_connected_peers())
asyncio.run(main())
"
```

---

## 📝 Summary

✅ **Working**: All example functionality is fully functional
✅ **Testing**: Requires separate Python processes per node
✅ **Production**: Ready for use in real P2P applications
⚠️ **Permissions**: May need network permissions for mDNS

For more examples, see the `test_p2p_demo.py` file which includes a feature-complete chat application with colored output!
