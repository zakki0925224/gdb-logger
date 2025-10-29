import socket
import json

SERVER_IP = "127.0.0.1"
SERVER_PORT = 6666


def udp_client(ip: str, port: int):
    client_socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    print(f"Connecting to {ip}:{port}")

    try:
        message = "abc"  # test message for tell ip address to server
        buf = message.encode("utf-8")
        client_socket.sendto(buf, (ip, port))
        print(f"[SEND ] {message}")
    except Exception as e:
        print(f"[ERROR] Send error: {e}")
        return

    try:
        while True:
            message, address = client_socket.recvfrom(2048)
            json_str = message.decode("utf-8")

            try:
                json_data = json.loads(json_str)
                print(f"[RECV ] {address}: {json_data}")
            except json.JSONDecodeError as e:
                print(f"[ERROR] JSON parse error: {e}")
    except KeyboardInterrupt:
        print("\nReception stopped.")
    except Exception as e:
        print(f"[ERROR] Receive error: {e}")
    finally:
        client_socket.close()
        print("Socket closed.")


if __name__ == "__main__":
    udp_client(SERVER_IP, SERVER_PORT)
