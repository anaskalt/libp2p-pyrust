import asyncio
import random
from aioconsole import ainput
import libp2p_pyrust as libp2p

# Callback function to handle received messages
def message_callback(message):
    sender, content = message.split(b':', 1)
    print(f"\nReceived message from {sender.decode('utf-8')}: {content.decode('utf-8')}")

# Asynchronous task to subscribe to messages
async def subscribe_task():
    libp2p.subscribe_to_messages(message_callback)

# Asynchronous task to handle user input and send messages
async def send_messages(peer_id):
    while True:
        user_input = await ainput('Enter message (or "quit" to exit): ')
        if user_input.lower() == 'quit':
            break
        message = f"{peer_id}:{user_input}".encode('utf-8')
        await libp2p.send_direct_message(peer_id, message)

# Main function to set up the network and handle messaging
async def main():
    # Generate a new keypair for the node
    key_file = f'node_{random.randint(1000, 9999)}.key'
    libp2p.generate_ed25519_keypair(key_file)

    # List of bootstrap nodes (you can add more if needed)
    bootnodes = ['/ip4/127.0.0.1/udp/5000/quic-v1/p2p/12D3KooWSYoEJBh6UtfAT8wdepcvH2sjVGUrSFjgsofZwvNWgFPe']

    # Initialize the P2P network
    await libp2p.init_global_p2p_network(bootnodes, 0, key_file, 'example-topic-name')

    # Give some time for the network to initialize
    await asyncio.sleep(5)

    # Retrieve and print the host addresses and peer ID
    host_addrs = await libp2p.get_host_addrs()
    peer_id = await libp2p.get_peer_id()
    print(f'Your Peer ID: {peer_id}')
    print('Your addresses:')
    for addr in host_addrs:
        print(f'  {addr}')

    # Give some time for the network to discover peers
    await asyncio.sleep(3)

    # Retrieve and print the connected peers
    connected_peers = await libp2p.get_global_connected_peers()
    print('Connected peers:')
    for peer in connected_peers:
        print(f'  {peer}')

    # Create a new asynchronous task to subscribe to messages
    asyncio.create_task(subscribe_task())

    # Ask the user which peer to communicate with
    target_peer = await ainput('Enter the Peer ID you want to communicate with: ')

    # Start the task to handle user input and send messages
    await send_messages(target_peer)

if __name__ == '__main__':
    asyncio.run(main())
