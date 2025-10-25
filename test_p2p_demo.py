#!/usr/bin/env python3
"""
P2P Chat Demo - Comprehensive Test
===================================
This script demonstrates the libp2p-pyrust library with:
- Automatic peer discovery
- Gossipsub pub/sub messaging
- Direct peer-to-peer messaging
- Real-time connection monitoring

Usage:
  Terminal 1 (Bootstrap Node):
    python test_p2p_demo.py --port 5000 --name "Node1"

  Terminal 2:
    python test_p2p_demo.py --port 5001 --name "Node2" --bootstrap /ip4/127.0.0.1/udp/5000/quic-v1/p2p/<PEER_ID>

  Terminal 3:
    python test_p2p_demo.py --port 5002 --name "Node3" --bootstrap /ip4/127.0.0.1/udp/5000/quic-v1/p2p/<PEER_ID>
"""

import asyncio
import argparse
import sys
import libp2p_pyrust as libp2p
from datetime import datetime

class Colors:
    HEADER = '\033[95m'
    BLUE = '\033[94m'
    CYAN = '\033[96m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    RED = '\033[91m'
    END = '\033[0m'
    BOLD = '\033[1m'

class P2PNode:
    def __init__(self, name, port, bootnodes):
        self.name = name
        self.port = port
        self.bootnodes = bootnodes
        self.peer_id = None
        self.running = True

    def print_banner(self):
        print(f"\n{Colors.CYAN}{'='*70}{Colors.END}")
        print(f"{Colors.BOLD}{Colors.BLUE}🚀 libp2p-pyrust P2P Node - {self.name}{Colors.END}")
        print(f"{Colors.CYAN}{'='*70}{Colors.END}\n")

    def message_callback(self, message):
        """Callback για όταν λαμβάνουμε broadcast μήνυμα"""
        timestamp = datetime.now().strftime("%H:%M:%S")
        try:
            msg = message.decode('utf-8')
            print(f"\n{Colors.GREEN}[{timestamp}] 📨 Broadcast: {msg}{Colors.END}")
            print(f"{Colors.YELLOW}> {Colors.END}", end='', flush=True)
        except:
            print(f"\n{Colors.RED}[{timestamp}] ⚠️  Received binary message{Colors.END}")
            print(f"{Colors.YELLOW}> {Colors.END}", end='', flush=True)

    async def subscribe_task(self):
        """Background task για message subscription"""
        await libp2p.subscribe_to_messages(self.message_callback)

    async def status_monitor(self):
        """Monitor για peer connections"""
        while self.running:
            await asyncio.sleep(15)
            try:
                peers = await libp2p.get_global_connected_peers()
                if peers:
                    print(f"\n{Colors.CYAN}ℹ️  Connected to {len(peers)} peer(s){Colors.END}")
                    print(f"{Colors.YELLOW}> {Colors.END}", end='', flush=True)
            except:
                pass

    async def send_messages(self):
        """Main loop για αποστολή μηνυμάτων"""
        print(f"{Colors.BOLD}Commands:{Colors.END}")
        print(f"  {Colors.GREEN}/peers{Colors.END}     - Show connected peers")
        print(f"  {Colors.GREEN}/addrs{Colors.END}     - Show my addresses")
        print(f"  {Colors.GREEN}/help{Colors.END}      - Show this help")
        print(f"  {Colors.GREEN}/quit{Colors.END}      - Exit")
        print(f"  {Colors.GREEN}<text>{Colors.END}     - Broadcast message to all peers\n")

        while self.running:
            try:
                print(f"{Colors.YELLOW}> {Colors.END}", end='', flush=True)
                user_input = await asyncio.get_event_loop().run_in_executor(None, sys.stdin.readline)
                user_input = user_input.strip()

                if not user_input:
                    continue

                if user_input == '/quit':
                    print(f"{Colors.RED}Shutting down...{Colors.END}")
                    self.running = False
                    break

                elif user_input == '/peers':
                    peers = await libp2p.get_global_connected_peers()
                    if peers:
                        print(f"\n{Colors.CYAN}Connected Peers ({len(peers)}):{Colors.END}")
                        for i, peer in enumerate(peers, 1):
                            print(f"  {i}. {peer}")
                    else:
                        print(f"{Colors.YELLOW}No peers connected yet{Colors.END}")

                elif user_input == '/addrs':
                    addrs = await libp2p.get_host_addrs()
                    print(f"\n{Colors.CYAN}My Addresses:{Colors.END}")
                    for addr in addrs:
                        print(f"  {addr}")

                elif user_input == '/help':
                    print(f"\n{Colors.BOLD}Commands:{Colors.END}")
                    print(f"  {Colors.GREEN}/peers{Colors.END}     - Show connected peers")
                    print(f"  {Colors.GREEN}/addrs{Colors.END}     - Show my addresses")
                    print(f"  {Colors.GREEN}/help{Colors.END}      - Show this help")
                    print(f"  {Colors.GREEN}/quit{Colors.END}      - Exit")
                    print(f"  {Colors.GREEN}<text>{Colors.END}     - Broadcast message\n")

                else:
                    # Broadcast message
                    message = f"[{self.name}] {user_input}"
                    await libp2p.publish_message(message.encode('utf-8'))
                    timestamp = datetime.now().strftime("%H:%M:%S")
                    print(f"{Colors.BLUE}[{timestamp}] ✓ Sent: {message}{Colors.END}")

            except EOFError:
                self.running = False
                break
            except Exception as e:
                print(f"{Colors.RED}Error: {e}{Colors.END}")

    async def run(self):
        """Κύρια συνάρτηση εκτέλεσης"""
        self.print_banner()

        # Δημιουργία keypair
        key_file = f'node_{self.name}.key'
        print(f"{Colors.CYAN}🔑 Generating keypair...{Colors.END}")
        libp2p.generate_ed25519_keypair(key_file)

        # Αρχικοποίηση P2P network
        print(f"{Colors.CYAN}🌐 Initializing P2P network on port {self.port}...{Colors.END}")
        await libp2p.init_global_p2p_network(
            self.bootnodes,
            self.port,
            key_file,
            'test-topic'
        )

        # Περιμένουμε να αρχικοποιηθεί το δίκτυο
        await asyncio.sleep(3)

        # Πληροφορίες node
        self.peer_id = await libp2p.get_peer_id()
        addrs = await libp2p.get_host_addrs()

        print(f"\n{Colors.GREEN}✓ Node initialized successfully!{Colors.END}")
        print(f"{Colors.BOLD}Peer ID:{Colors.END} {self.peer_id}")
        print(f"\n{Colors.BOLD}Listening on:{Colors.END}")
        for addr in addrs:
            print(f"  {addr}")
        print()

        # Περιμένουμε για peer discovery
        if self.bootnodes:
            print(f"{Colors.CYAN}🔍 Discovering peers...{Colors.END}")
            await asyncio.sleep(5)

            peers = await libp2p.get_global_connected_peers()
            if peers:
                print(f"{Colors.GREEN}✓ Connected to {len(peers)} peer(s){Colors.END}\n")
            else:
                print(f"{Colors.YELLOW}⚠️  No peers connected yet (they may connect later){Colors.END}\n")

        # Start background tasks
        asyncio.create_task(self.subscribe_task())
        asyncio.create_task(self.status_monitor())

        # Main message loop
        await self.send_messages()

async def main():
    parser = argparse.ArgumentParser(description='P2P Chat Node')
    parser.add_argument('--name', type=str, required=True, help='Node name')
    parser.add_argument('--port', type=int, required=True, help='Port to listen on')
    parser.add_argument('--bootstrap', type=str, action='append', default=[],
                       help='Bootstrap node address (can be specified multiple times)')

    args = parser.parse_args()

    node = P2PNode(args.name, args.port, args.bootstrap)

    try:
        await node.run()
    except KeyboardInterrupt:
        print(f"\n{Colors.RED}Interrupted by user{Colors.END}")
    except Exception as e:
        print(f"{Colors.RED}Error: {e}{Colors.END}")
        import traceback
        traceback.print_exc()

if __name__ == '__main__':
    asyncio.run(main())
