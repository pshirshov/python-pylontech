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

def to_json_serializable(obj):
    from io import BytesIO
    from construct import Container
    import base64

    if isinstance(obj, Container):
        return {str(k): to_json_serializable(v) for k, v in obj.items() if k != "_io"}
    elif isinstance(obj, dict):
        return {str(k): to_json_serializable(v) for k, v in obj.items() if k != "_io"}
    elif isinstance(obj, list):
        return [to_json_serializable(v) for v in obj]
    elif isinstance(obj, BytesIO):
        return base64.b64encode(obj.getvalue()).decode('utf-8')  # or use .hex()
    elif isinstance(obj, bytes):
        return base64.b64encode(obj).decode('utf-8')  # or use obj.hex()
    elif hasattr(obj, '__dict__'):
        return {str(k): to_json_serializable(v) for k, v in vars(obj).items()}
    else:
        return obj
