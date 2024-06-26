import asyncio

from aioconsole import ainput

import libp2p_pyrust as libp2p


# Callback function to handle received messages
def my_callback(message):
    print(f"Received message: {message.decode('utf-8')}")


# Asynchronous task to subscribe to messages
async def subscribe_task():
    libp2p.subscribe_to_messages(my_callback)


# Asynchronous task to handle user input and send messages
async def send_messages():
    while True:
        user_input = await ainput('Enter message: ')
        await libp2p.publish_message(user_input.encode('utf-8'))


# Main function to set up the network and handle messaging
async def main():
    # Generate a new keypair for the node
    libp2p.generate_ed25519_keypair('node.key')

    # List of bootstrap nodes
    bootnodes = ['/ip4/127.0.0.1/udp/5000/quic-v1/p2p/12D3KooWSYoEJBh6UtfAT8wdepcvH2sjVGUrSFjgsofZwvNWgFPe']

    # Initialize the P2P network
    await libp2p.init_global_p2p_network(bootnodes, 0, 'node.key', 'example-topic-name')

    # Give some time for the network to initialize
    await asyncio.sleep(10)

    # Retrieve and print the host addresses
    host_addrs = await libp2p.get_host_addrs()
    print('Hosts: ', host_addrs)

    # Give some time for the network to discover peers
    await asyncio.sleep(3)

    # Retrieve and print the connected peers
    connected_peers = await libp2p.get_global_connected_peers()
    print('Connected peers:', connected_peers)

    # Create a new asynchronous task to subscribe to messages
    asyncio.create_task(subscribe_task())

    # Start the task to handle user input and send messages
    await send_messages()


if __name__ == '__main__':
    asyncio.run(main())
