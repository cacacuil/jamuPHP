#!/usr/bin/env python3
"""Server SMTP palsu untuk uji mode teruskan email.

Pemakaian: smtp_palsu.py PORT BERKAS

Menerima email di 127.0.0.1:PORT tanpa TLS dan mencatat amplop serta isinya ke BERKAS.
"""
import socketserver
import sys

port, berkas = int(sys.argv[1]), sys.argv[2]


def catat(baris):
    with open(berkas, "a", encoding="utf-8", errors="replace") as f:
        f.write(baris + "\n")


class Smtp(socketserver.StreamRequestHandler):
    def kirim(self, teks):
        self.wfile.write(teks.encode() + b"\r\n")

    def handle(self):
        self.kirim("220 palsu ESMTP")
        while True:
            baris = self.rfile.readline()
            if not baris:
                return
            baris = baris.decode("utf-8", "replace").rstrip("\r\n")
            perintah = baris.upper()
            if perintah.startswith(("EHLO", "HELO")):
                self.wfile.write(b"250-palsu\r\n")
                self.kirim("250 8BITMIME")
            elif perintah.startswith(("MAIL FROM", "RCPT TO")):
                catat(baris)
                self.kirim("250 ok")
            elif perintah == "DATA":
                self.kirim("354 kirim")
                while True:
                    isi = self.rfile.readline()
                    if not isi or isi.rstrip(b"\r\n") == b".":
                        break
                    catat("| " + isi.decode("utf-8", "replace").rstrip("\r\n"))
                catat("SELESAI")
                self.kirim("250 diterima")
            elif perintah == "QUIT":
                self.kirim("221 dah")
                return
            else:
                self.kirim("250 ok")


socketserver.ThreadingTCPServer.allow_reuse_address = True
with socketserver.ThreadingTCPServer(("127.0.0.1", port), Smtp) as server:
    server.serve_forever()
