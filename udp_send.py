import socket
import sys

try:
    PORT = int(sys.argv[1])
except IndexError:
    PORT = int(input("port: "))

SOCK = socket.socket(type=socket.SOCK_DGRAM)
SOCK.connect(("127.0.0.1", PORT))
DATA = b"users.online:1|c|@0.5|#country:china"
DATA = b"\n".join([DATA] * 221)
while True:
    SOCK.send(DATA)
