from __future__ import annotations

import ctypes
import uuid


class GUID(ctypes.Structure):
    _fields_ = [("Data1", ctypes.c_uint32), ("Data2", ctypes.c_uint16), ("Data3", ctypes.c_uint16), ("Data4", ctypes.c_ubyte * 8)]

    @classmethod
    def from_str(cls, text: str) -> GUID:
        u = uuid.UUID(text)
        b = u.bytes_le
        return cls.from_buffer_copy(b)


def vtable_fn(ptr: ctypes.c_void_p, index: int, restype, *argtypes):
    vtbl = ctypes.cast(ptr, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p))).contents
    fn_type = ctypes.WINFUNCTYPE(restype, ctypes.c_void_p, *argtypes)
    return fn_type(vtbl[index])


def com_release(ptr: ctypes.c_void_p | None) -> None:
    if ptr and ptr.value:
        vtable_fn(ptr, 2, ctypes.c_ulong)(ptr)
