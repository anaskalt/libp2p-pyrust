import asyncio
import random
from aioconsole import ainput
import libp2p_pyrust as libp2p

# Callback function to handle received messages
def message_callback(message):
    # The first 52 characters of the message should be the sender's PeerId
    sender = message[:52].decode('utf-8')
    content = message[52:].decode('utf-8')
    print(f"\nReceived message from {sender}: {content}")

# Asynchronous task to subscribe to messages
async def subscribe_task():
    libp2p.subscribe_to_messages(message_callback)

# Asynchronous task to handle user input and send messages
async def send_messages(connected_peers, our_peer_id):
    current_peer = None
    while True:
        if current_peer:
            user_input = await ainput(f'Enter message for {current_peer} (or "switch" to change peer, "quit" to exit): ')
        else:
            user_input = await ainput('Enter "switch" to select a peer or "quit" to exit: ')

        if user_input.lower() == 'quit':
            break
        elif user_input.lower() == 'switch':
            print("Connected peers:")
            for i, peer in enumerate(connected_peers):
                print(f"{i + 1}. {peer}")
            peer_choice = await ainput("Enter the number of the peer you want to message: ")
            try:
                peer_index = int(peer_choice) - 1
                if 0 <= peer_index < len(connected_peers):
                    current_peer = connected_peers[peer_index]
                    print(f"Now messaging: {current_peer}")
                else:
                    print("Invalid peer number. Please try again.")
            except ValueError:
                print("Invalid input. Please enter a number.")
        elif current_peer:
            # Prepend our PeerId to the message
            message = f"{our_peer_id}{user_input}".encode('utf-8')
            await libp2p.send_direct_message(current_peer, message)
            print(f"Message sent to {current_peer}")
        else:
            print("Please select a peer first using the 'switch' command.")

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
    our_peer_id = await libp2p.get_peer_id()
    print(f'Your Peer ID: {our_peer_id}')
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

    # Start the task to handle user input and send messages
    await send_messages(connected_peers, our_peer_id)

if __name__ == '__main__':
    asyncio.run(main())
