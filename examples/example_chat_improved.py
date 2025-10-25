#!/usr/bin/env python3
"""
Improved Chat Example - Easy to use with command line arguments

Usage:
  # Terminal 1 (Bootstrap node):
  python3 example_chat_improved.py Alice 5000

  # Terminal 2 (Connecting node - use address from Terminal 1):
  python3 example_chat_improved.py Bob 5001 /ip4/127.0.0.1/udp/5000/quic-v1/p2p/<PEER_ID>

  # Terminal 3:
  python3 example_chat_improved.py Charlie 5002 /ip4/127.0.0.1/udp/5000/quic-v1/p2p/<PEER_ID>

Type messages and press Enter to broadcast to all connected peers.
Type '/quit' to exit.
"""

import asyncio
import sys
from aioconsole import ainput
import libp2p_pyrust as libp2p


def my_callback(message):
    """Callback for received messages"""
    print(f"\n📨 {message.decode('utf-8')}")
    print("Message: ", end='', flush=True)


async def subscribe_task():
    """Background task for message subscription"""
    await libp2p.subscribe_to_messages(my_callback)


async def send_messages(node_name):
    """Interactive message sending"""
    while True:
        user_input = await ainput('Message: ')

        if user_input.lower() == '/quit':
            print("👋 Goodbye!")
            break
        elif user_input.lower() == '/peers':
            peers = await libp2p.get_global_connected_peers()
            print(f"📊 Connected to {len(peers)} peer(s):")
            for peer in peers:
                print(f"  - {peer}")
            continue
        elif user_input.lower() == '/help':
            print("\nCommands:")
            print("  /quit   - Exit")
            print("  /peers  - Show connected peers")
            print("  /help   - Show this help")
            print("  <text>  - Send message to all peers\n")
            continue

        # Send message with sender name
        msg = f"[{node_name}] {user_input}"
        await libp2p.publish_message(msg.encode('utf-8'))


async def main():
    # Parse command line arguments
    if len(sys.argv) < 3:
        print(__doc__)
        return 1

    node_name = sys.argv[1]
    port = int(sys.argv[2])
    bootstrap = sys.argv[3] if len(sys.argv) > 3 else None

    print(f"\n{'='*70}")
    print(f"🚀 Starting {node_name} on port {port}")
    print(f"{'='*70}\n")

    # Generate keypair
    key_file = f'node_{node_name}.key'
    libp2p.generate_ed25519_keypair(key_file)
    print(f"✓ Keypair: {key_file}")

    # Initialize network
    bootnodes = [bootstrap] if bootstrap else []
    print(f"✓ Initializing P2P network...")

    await libp2p.init_global_p2p_network(
        bootnodes,
        port,
        key_file,
        'example-chat-topic'
    )

    await asyncio.sleep(3)

    # Show node info
    peer_id = await libp2p.get_peer_id()
    addrs = await libp2p.get_host_addrs()

    print(f"✓ Peer ID: {peer_id}")
    print(f"✓ Listening on {len(addrs)} address(es)")

    if not bootstrap:
        print("\n📍 Bootstrap address (use this in other terminals):")
        for addr in addrs:
            if 'quic-v1' in addr:
                print(f"  {addr}")

    # Wait for peer discovery
    print("\n🔍 Discovering peers...")
    await asyncio.sleep(5)

    peers = await libp2p.get_global_connected_peers()
    if peers:
        print(f"✓ Connected to {len(peers)} peer(s)")
    else:
        print("⚠️  No peers connected yet (they may connect later)")

    print(f"\n{'='*70}")
    print("💬 Chat ready! Type your messages below.")
    print("   Commands: /quit, /peers, /help")
    print(f"{'='*70}\n")

    # Start messaging
    asyncio.create_task(subscribe_task())

    try:
        await send_messages(node_name)
    except KeyboardInterrupt:
        print("\n\n👋 Interrupted - goodbye!")

    return 0


if __name__ == '__main__':
    exit_code = asyncio.run(main())
    sys.exit(exit_code or 0)
