import logging

import serial
import telnetlib

from .tools import *

logger = logging.getLogger(__name__)

class ChecksumMismatch(Exception):
    def __init__(self, expected, actual):
        self.expected = expected
        self.actual = actual
        super().__init__(self.__repr__())

    def __repr__(self):
        return f"expected {self.expected}, got {self.actual}"

class FrameFormatException(Exception):
    def __init__(self, raw_frame, message, cause = None):
        self.raw_frame = raw_frame
        self.cause = cause
        self.message = message
        super().__init__(self.__repr__())

    def __repr__(self):
        return self.message


class SerialTransport():
    def readln(self) -> bytes:
        pass

    def write(self, data: bytes):
        pass

    def send_cmd(self, address: int, cmd, info: bytes = b''):
        raw_frame = self._encode_cmd(address, cmd, info)
        self.write(raw_frame)

    def read_frame(self):
        raw_frame = self.readln()
        f = self._decode_hw_frame(raw_frame=raw_frame)
        parsed = self._decode_frame(f)
        return parsed

    def _encode_cmd(self, address: int, cid2: int, info: bytes = b''):
        cid1 = 0x46

        info_length = SerialTransport.get_info_length(info)

        frame = "{:02X}{:02X}{:02X}{:02X}{:04X}".format(0x20, address, cid1, cid2, info_length).encode()
        frame += info

        frame_chksum = SerialTransport.get_frame_checksum(frame)
        whole_frame = (b"~" + frame + "{:04X}".format(frame_chksum).encode() + b"\r")
        return whole_frame


    def _decode_hw_frame(self, raw_frame: bytes) -> bytes:
        try:
            frame_data = raw_frame[1:len(raw_frame) - 5]
            frame_chksum = raw_frame[len(raw_frame) - 5:-1]
            expected_frame_checksum = int(frame_chksum, 16)
            real_frame_checksum = SerialTransport.get_frame_checksum(frame_data)
        except BaseException as e:
            m=f"cannot decode frame bytes, frame {raw_frame}"
            raise FrameFormatException(raw_frame, message=m, cause=e)

        if real_frame_checksum != expected_frame_checksum:
            m = f"expected checksum {expected_frame_checksum}, got {real_frame_checksum}, frame {raw_frame}"
            raise FrameFormatException(raw_frame, message=m, cause=ChecksumMismatch(expected_frame_checksum, real_frame_checksum))

        return frame_data

    @staticmethod
    def get_frame_checksum(frame: bytes):
        assert isinstance(frame, bytes)

        sum = 0
        for byte in frame:
            sum += byte
        sum = ~sum
        sum %= 0x10000
        sum += 1
        return sum

    @staticmethod
    def get_info_length(info: bytes) -> int:
        lenid = len(info)
        if lenid == 0:
            return 0

        lenid_sum = (lenid & 0xf) + ((lenid >> 4) & 0xf) + ((lenid >> 8) & 0xf)
        lenid_modulo = lenid_sum % 16
        lenid_invert_plus_one = 0b1111 - lenid_modulo + 1

        return (lenid_invert_plus_one << 12) + lenid

    def _decode_frame(self, frame):
        format = construct.Struct(
            "ver" / HexToByte(construct.Array(2, construct.Byte)),
            "adr" / HexToByte(construct.Array(2, construct.Byte)),
            "cid1" / HexToByte(construct.Array(2, construct.Byte)),
            "cid2" / HexToByte(construct.Array(2, construct.Byte)),
            "infolength" / HexToByte(construct.Array(4, construct.Byte)),
            "info" / HexToByte(construct.GreedyRange(construct.Byte)),
        )

        return format.parse(frame)

class SerialDeviceTransport(SerialTransport):
    def __init__(self, serial_port='/dev/ttyUSB0', baudrate=115200):
        self.s = serial.Serial(serial_port, baudrate, bytesize=8, parity=serial.PARITY_NONE, stopbits=1, timeout=2, exclusive=True)

    def readln(self) -> bytes:
        return self.s.readline()

    def write(self, data: bytes):
        self.s.write(data)


class TelnetlibLegacyTransport(SerialTransport):
    def __init__(self, host, port=23, timeout=2):
        self.timeout = timeout
        self.s = telnetlib.Telnet(host, port, timeout=self.timeout)

    def readln(self) -> bytes:
        return self.s.read_until(b'\r', timeout=self.timeout)

    def write(self, data: bytes):
        self.s.write(data)

from Exscript.protocols import Telnet

class ExscriptTelnetTransport(SerialTransport):
    def __init__(self, host, port=23, timeout=2):
        self.timeout = timeout
        self.conn = Telnet()
        self.conn.connect(host, port)
        self.conn.set_timeout(timeout)

    def readln(self):
        data = b''
        while True:
            chunk = self.conn.tn.rawq_getchar()
            if not chunk:
                break
            data += chunk
            if chunk == b'\r':
                break
        return data

    def write(self, data: bytes):
        self.conn.send(data)
