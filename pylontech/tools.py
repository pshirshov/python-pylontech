import construct


class HexToByte(construct.Adapter):
    def _decode(self, obj, context, path) -> bytes:
        hexstr = ''.join([chr(x) for x in obj])
        return bytes.fromhex(hexstr)


class JoinBytes(construct.Adapter):
    def _decode(self, obj, context, path) -> bytes:
        return ''.join([chr(x) for x in obj]).encode()


class DivideBy1000(construct.Adapter):
    def _decode(self, obj, context, path) -> float:
        return obj / 1000


class DivideBy100(construct.Adapter):
    def _decode(self, obj, context, path) -> float:
        return obj / 100

class DivideBy10(construct.Adapter):
    def _decode(self, obj, context, path) -> float:
        return obj / 10

class ToVolt(construct.Adapter):
    def _decode(self, obj, context, path) -> float:
        return obj / 1000

class ToAmp(construct.Adapter):
    def _decode(self, obj, context, path) -> float:
        return obj / 10

class ToCelsius(construct.Adapter):
    def _decode(self, obj, context, path) -> float:
        return (obj - 2731) / 10.0  # in Kelvin*10


