#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Host-side keeberry config protocol (kcp) probe over raw-HID.

kcp is keeberry's single configuration surface: one fixed 32-byte raw-HID report
per message on a QMK-style vendor interface (usage page ``0xFF60``, usage
``0x61``). The bytes are identical over every link, so this tool speaks the same
kcp whether it opens the keyboard's own USB interface or the 2.4 GHz dongle's
bridged one -- which is exactly what makes it useful for diagnosing why the
configurator connects over USB but not over the dongle.

Framing (mirrors ``firmware/src/kcp.rs``)::

    request : [0]=CMD       [1]=SEQ  [2..32]=payload (30 bytes)
    reply   : [0]=CMD|0x80  [1]=SEQ  [2]=STATUS  [3..32]=payload (29 bytes)

The interface carries no HID report ID, so on the host an OUT report is written
as ``[0x00] + 32 data bytes`` (hidapi requires a leading report-id byte; here it
is always 0) and an IN report reads back as the 32 data bytes with no prefix.

Because the kcp interface is opened exclusively, close any browser tab or native
app that holds the device first -- otherwise ``open_path`` fails or every read
times out.

Examples::

    # List every kcp (0xFF60/0x61) interface currently on the bus.
    python3 tools/kcp_cli.py enumerate

    # Talk to the keeberry keyboard over USB (default VID/PID 0x1209/0x0001).
    python3 tools/kcp_cli.py info
    python3 tools/kcp_cli.py wls-state

    # Talk to whatever device exposes kcp, e.g. the Akko-VID dongle, by matching
    # any VID/PID that carries the 0xFF60/0x61 usage.
    python3 tools/kcp_cli.py --any wls-state
    python3 tools/kcp_cli.py --vid 0x342d --pid 0xe4d7 get-protocol

    # Drive a 2.4 GHz pairing from the (USB-connected) keyboard, then re-scan.
    python3 tools/kcp_cli.py wls-mode 2g4
    python3 tools/kcp_cli.py wls-pair
    python3 tools/kcp_cli.py enumerate

    # One-shot end-to-end diagnosis (USB -> switch 2.4G -> pair -> re-enumerate).
    python3 tools/kcp_cli.py diagnose
"""

from __future__ import annotations

import argparse
import sys
import time
from dataclasses import dataclass
from typing import Optional

import hid

# --- Wire constants (source of truth: firmware/src/kcp.rs, app/src/kcp/protocol.ts) ---

USAGE_PAGE = 0xFF60
USAGE = 0x61

# keeberry's shipping USB identity (pid.codes TEST allocation), mirrored in the
# firmware's usb.rs and the app's webhid-transport.ts / hid.rs.
KEEBERRY_VID = 0x1209
KEEBERRY_PID = 0x0001

MSG_LEN = 32
REPLY_FLAG = 0x80

# Reply STATUS byte (kcp.rs `enum Status`).
STATUS_LABELS = {0: "Ok", 1: "BadCmd", 2: "BadArg", 3: "Busy", 4: "Unsupported"}

# INFO group (0x0x).
CMD_GET_VERSION = 0x00
CMD_GET_CAPABILITIES = 0x01
CMD_GET_DEVICE_INFO = 0x02

# WIRELESS group (0x8x).
CMD_WLS_GET_STATE = 0x80
CMD_WLS_SET_MODE = 0x81
CMD_WLS_PAIR = 0x82
CMD_WLS_UNPAIR = 0x83
CMD_WLS_SET_SLEEP_POLICY = 0x84
CMD_WLS_GET_BATTERY = 0x85
CMD_CONFIG_SAVE = 0x40
CMD_CONFIG_GET_STORAGE_INFO = 0x42

# wireless::Devs codes (firmware/src/wireless/mod.rs). USB=0, BT1..3=1..3, 2.4G=6.
DEVS_BY_NAME = {"usb": 0, "bt1": 1, "bt2": 2, "bt3": 3, "2g4": 6}
DEVS_LABELS = {0: "USB", 1: "Bluetooth 1", 2: "Bluetooth 2", 3: "Bluetooth 3", 6: "2.4 GHz"}

# Radio connection state (wireless/md.rs MdState).
CONN_STATE_LABELS = {
    0: "Idle/None",
    1: "Pairing",
    2: "Connected",
    3: "Disconnected",
    4: "Rejected",
}


class KcpError(Exception):
    """A kcp exchange failed (no device, timeout, or a non-OK STATUS byte)."""


@dataclass
class HidMatch:
    """One enumerated kcp interface: the fields needed to open and report it."""

    path: bytes
    vendor_id: int
    product_id: int
    manufacturer: str
    product: str

    def describe(self) -> str:
        return (
            f"VID={self.vendor_id:#06x} PID={self.product_id:#06x} "
            f"manufacturer={self.manufacturer!r} product={self.product!r}"
        )


def find_kcp_interfaces(
    vid: Optional[int] = None, pid: Optional[int] = None
) -> list[HidMatch]:
    """Enumerate every HID interface exposing the kcp usage (``0xFF60``/``0x61``).

    ``vid``/``pid`` narrow the match when given; ``None`` for either matches any
    value, so ``find_kcp_interfaces()`` finds the keyboard *and* the dongle
    regardless of whose VID/PID the dongle currently advertises. Matching on the
    usage page/usage (not just VID/PID) is what lets the tool open the correct
    collection on macOS, where one physical device exposes several.
    """
    matches: list[HidMatch] = []
    for info in hid.enumerate():
        if info.get("usage_page") != USAGE_PAGE or info.get("usage") != USAGE:
            continue
        if vid is not None and info["vendor_id"] != vid:
            continue
        if pid is not None and info["product_id"] != pid:
            continue
        matches.append(
            HidMatch(
                path=info["path"],
                vendor_id=info["vendor_id"],
                product_id=info["product_id"],
                manufacturer=info.get("manufacturer_string") or "",
                product=info.get("product_string") or "",
            )
        )
    return matches


class KcpDevice:
    """An open kcp interface with request/reply transaction helpers.

    Opens exactly the ``0xFF60``/``0x61`` collection (by OS path, so the keyboard
    collection and not, say, its usage-page ``0x01`` keyboard collection) and
    exchanges 32-byte frames. Use as a context manager so the handle is always
    released -- a leaked handle blocks the configurator from reconnecting.
    """

    def __init__(self, match: HidMatch):
        self.match = match
        self._dev = hid.device()
        self._seq = 0

    def __enter__(self) -> "KcpDevice":
        self._dev.open_path(self.match.path)
        # Blocking reads with an explicit per-read timeout (see `transact`); the
        # firmware answers in well under a millisecond over USB, but the 2.4 GHz
        # bridge adds the radio round-trip and a possible wake, so timeouts are
        # generous and set per call.
        self._dev.set_nonblocking(0)
        return self

    def __exit__(self, *_exc) -> None:
        self._dev.close()

    def _next_seq(self) -> int:
        seq = self._seq
        self._seq = (self._seq + 1) & 0xFF
        return seq

    def transact(
        self, cmd: int, payload: Optional[bytes] = None, timeout_ms: int = 2000
    ) -> bytes:
        """Send one request and return the matching reply's 32-byte frame.

        Writes ``[CMD, SEQ, payload...]`` (zero-padded to 32 data bytes, prefixed
        with the mandatory report-id 0) and reads inbound frames until one both
        echoes ``SEQ`` and carries ``CMD | REPLY_FLAG`` -- the same pairing rule
        the app's `KcpConnection` uses -- ignoring any unsolicited frames the
        dongle bridge may interleave. Raises {@link KcpError} on timeout.
        """
        seq = self._next_seq()
        frame = bytearray(MSG_LEN)
        frame[0] = cmd & 0xFF
        frame[1] = seq
        if payload:
            if len(payload) > MSG_LEN - 2:
                raise ValueError("payload exceeds the 30-byte request region")
            frame[2 : 2 + len(payload)] = payload
        # hidapi's write() takes the report id as byte 0; this interface is
        # unnumbered, so it is always 0.
        self._dev.write(bytes([0x00]) + bytes(frame))

        want_cmd = cmd | REPLY_FLAG
        deadline = time.monotonic() + timeout_ms / 1000.0
        while True:
            remaining_ms = int((deadline - time.monotonic()) * 1000)
            if remaining_ms <= 0:
                raise KcpError(
                    f"timed out waiting for reply to cmd={cmd:#04x} seq={seq}"
                )
            data = self._dev.read(MSG_LEN, timeout_ms=remaining_ms)
            if not data:
                continue
            reply = bytes(data)
            if len(reply) < 3:
                continue
            if reply[1] == seq and reply[0] == want_cmd:
                return reply
            # Unsolicited or mismatched frame (e.g. another in-flight reply on the
            # shared bridge): keep draining until ours arrives or time runs out.

    def transact_ok(
        self, cmd: int, payload: Optional[bytes] = None, timeout_ms: int = 2000
    ) -> bytes:
        """`transact`, but raise unless STATUS is Ok; return the reply payload."""
        reply = self.transact(cmd, payload, timeout_ms)
        status = reply[2]
        if status != 0:
            label = STATUS_LABELS.get(status, f"Unknown({status})")
            raise KcpError(f"cmd={cmd:#04x} returned STATUS={label}")
        return reply[3:]

    # --- Typed command wrappers ------------------------------------------------

    def get_protocol(self, timeout_ms: int = 2000) -> tuple[int, int]:
        """INFO 0x00 -- protocol version ``(major, minor)``; the sanity ping."""
        payload = self.transact_ok(CMD_GET_VERSION, timeout_ms=timeout_ms)
        return payload[0], payload[1]

    def get_capabilities(self, timeout_ms: int = 2000) -> int:
        """INFO 0x01 -- capabilities bitmask (little-endian u32)."""
        payload = self.transact_ok(CMD_GET_CAPABILITIES, timeout_ms=timeout_ms)
        return int.from_bytes(payload[:4], "little")

    def get_device_info(self, timeout_ms: int = 2000) -> dict:
        """INFO 0x02 -- static device descriptor (see kcp.rs pack_device_info)."""
        p = self.transact_ok(CMD_GET_DEVICE_INFO, timeout_ms=timeout_ms)
        return {
            "firmware": f"{p[0]}.{p[1]}.{p[2]}",
            "chip": bytes(p[3:11]).rstrip(b"\x00").decode("ascii", "replace"),
            "rows": p[11],
            "cols": p[12],
            "layers": p[13],
            "transport": p[14],
            "transport_label": DEVS_LABELS.get(p[14], f"Unknown({p[14]})"),
            "schema": int.from_bytes(p[15:17], "little"),
        }

    def wls_get_state(self, timeout_ms: int = 2000) -> dict:
        """WIRELESS 0x80 -- link snapshot ``[devs, state, battery, version]``."""
        p = self.transact_ok(CMD_WLS_GET_STATE, timeout_ms=timeout_ms)
        return {
            "devs": p[0],
            "devs_label": DEVS_LABELS.get(p[0], f"Unknown({p[0]})"),
            "state": p[1],
            "state_label": CONN_STATE_LABELS.get(p[1], f"Unknown({p[1]})"),
            "battery": p[2],
            "version": p[3],
        }

    def wls_set_mode(self, devs: int, timeout_ms: int = 2000) -> None:
        """WIRELESS 0x81 -- select the output transport (an unknown code -> BadArg)."""
        self.transact_ok(CMD_WLS_SET_MODE, bytes([devs & 0xFF]), timeout_ms=timeout_ms)

    def wls_pair(self, timeout_ms: int = 2000) -> None:
        """WIRELESS 0x82 -- (re)pair the current transport (reset = true)."""
        self.transact_ok(CMD_WLS_PAIR, timeout_ms=timeout_ms)

    def wls_unpair(self, timeout_ms: int = 2000) -> None:
        """WIRELESS 0x83 -- clear the active channel's bond."""
        self.transact_ok(CMD_WLS_UNPAIR, timeout_ms=timeout_ms)

    def wls_get_battery(self, timeout_ms: int = 2000) -> int:
        """WIRELESS 0x85 -- battery percent (also triggers a fresh measurement)."""
        return self.transact_ok(CMD_WLS_GET_BATTERY, timeout_ms=timeout_ms)[0]

    def config_storage_info(self, timeout_ms: int = 2000) -> dict:
        """CONFIG 0x42 -- persistence descriptor ``[base u32, size u32, version u16, valid u8]``."""
        p = self.transact_ok(CMD_CONFIG_GET_STORAGE_INFO, timeout_ms=timeout_ms)
        return {
            "base": int.from_bytes(p[0:4], "little"),
            "size": int.from_bytes(p[4:8], "little"),
            "version": int.from_bytes(p[8:10], "little"),
            "valid": bool(p[10]),
        }

    def config_save(self, timeout_ms: int = 2000) -> None:
        """CONFIG 0x40 -- persist the complete live state to flash."""
        self.transact_ok(CMD_CONFIG_SAVE, timeout_ms=timeout_ms)


# --- Device selection ---------------------------------------------------------


def select_device(args) -> HidMatch:
    """Resolve CLI selection flags to exactly one kcp interface.

    Defaults to the keeberry VID/PID; ``--any`` drops the VID/PID constraint (to
    reach a dongle still advertising Akko's identity), and ``--vid``/``--pid``
    pin explicit values. Fails loudly when zero or several match so a probe never
    silently talks to the wrong board.
    """
    if args.any:
        vid = pid = None
    else:
        vid = args.vid if args.vid is not None else KEEBERRY_VID
        pid = args.pid if args.pid is not None else KEEBERRY_PID
    matches = find_kcp_interfaces(vid, pid)
    if not matches:
        scope = "any VID/PID" if args.any else f"VID={vid:#06x} PID={pid:#06x}"
        raise KcpError(
            f"no kcp (0xFF60/0x61) interface found for {scope}. "
            "Is the device connected and not held by a browser/app?"
        )
    if len(matches) > 1:
        listing = "\n".join(f"  - {m.describe()}" for m in matches)
        raise KcpError(
            "multiple kcp interfaces match; narrow with --vid/--pid:\n" + listing
        )
    return matches[0]


# --- Subcommand handlers ------------------------------------------------------


def cmd_enumerate(_args) -> int:
    matches = find_kcp_interfaces()
    if not matches:
        print("No kcp (0xFF60/0x61) interfaces found.")
        return 1
    print(f"Found {len(matches)} kcp interface(s):")
    for m in matches:
        keeberry = m.vendor_id == KEEBERRY_VID and m.product_id == KEEBERRY_PID
        tag = "  [matches app filter]" if keeberry else "  [REJECTED by app filter]"
        print(f"  - {m.describe()}{tag}")
    return 0


def cmd_get_protocol(args) -> int:
    with KcpDevice(select_device(args)) as dev:
        major, minor = dev.get_protocol(timeout_ms=args.timeout)
        print(f"kcp protocol version: {major}.{minor}  (via {dev.match.describe()})")
    return 0


def cmd_info(args) -> int:
    with KcpDevice(select_device(args)) as dev:
        print(f"device: {dev.match.describe()}")
        major, minor = dev.get_protocol(timeout_ms=args.timeout)
        print(f"  protocol : {major}.{minor}")
        caps = dev.get_capabilities(timeout_ms=args.timeout)
        print(f"  caps     : {caps:#06x}")
        info = dev.get_device_info(timeout_ms=args.timeout)
        print(
            f"  firmware : {info['firmware']}  chip={info['chip']}  "
            f"matrix={info['rows']}x{info['cols']}  layers={info['layers']}"
        )
        print(
            f"  transport: {info['transport']} ({info['transport_label']})  "
            f"schema={info['schema']}"
        )
    return 0


def cmd_wls_state(args) -> int:
    with KcpDevice(select_device(args)) as dev:
        s = dev.wls_get_state(timeout_ms=args.timeout)
        print(
            f"wireless: mode={s['devs']} ({s['devs_label']})  "
            f"state={s['state']} ({s['state_label']})  "
            f"battery={s['battery']}%  radio=v{s['version']}"
        )
    return 0


def cmd_wls_mode(args) -> int:
    devs = DEVS_BY_NAME.get(args.mode.lower())
    if devs is None:
        print(f"unknown mode {args.mode!r}; choose one of {list(DEVS_BY_NAME)}")
        return 2
    with KcpDevice(select_device(args)) as dev:
        dev.wls_set_mode(devs, timeout_ms=args.timeout)
        print(f"set output transport -> {DEVS_LABELS[devs]}")
        s = dev.wls_get_state(timeout_ms=args.timeout)
        print(f"  now: mode={s['devs_label']}  state={s['state_label']}")
    return 0


def cmd_wls_pair(args) -> int:
    with KcpDevice(select_device(args)) as dev:
        dev.wls_pair(timeout_ms=args.timeout)
        print("pairing started for the current transport (reset = true)")
        s = dev.wls_get_state(timeout_ms=args.timeout)
        print(f"  now: mode={s['devs_label']}  state={s['state_label']}")
    return 0


def cmd_wls_battery(args) -> int:
    with KcpDevice(select_device(args)) as dev:
        print(f"battery: {dev.wls_get_battery(timeout_ms=args.timeout)}%")
    return 0


def cmd_config_info(args) -> int:
    with KcpDevice(select_device(args)) as dev:
        i = dev.config_storage_info(timeout_ms=args.timeout)
        print(
            f"config region: base=0x{i['base']:08x}  size={i['size']} B  "
            f"version={i['version']}  valid={i['valid']}"
        )
    return 0


def cmd_config_save(args) -> int:
    with KcpDevice(select_device(args)) as dev:
        dev.config_save(timeout_ms=args.timeout)
        i = dev.config_storage_info(timeout_ms=args.timeout)
        print(f"    saved -> base=0x{i['base']:08x}  version={i['version']}  valid={i['valid']}")
    return 0


def _snapshot_kcp_bus() -> None:
    """Print every kcp interface on the bus and whether the app filter accepts it."""
    for m in find_kcp_interfaces():
        keeberry = m.vendor_id == KEEBERRY_VID and m.product_id == KEEBERRY_PID
        tag = "[matches app filter]" if keeberry else "[REJECTED by app filter]"
        print(f"    {m.describe()}  {tag}")


def cmd_diagnose(args) -> int:
    """End-to-end 2.4 GHz diagnosis, driven from the USB-connected keyboard.

    Confirms kcp over USB, switches the output transport to 2.4 GHz and pairs,
    then re-enumerates the bus to see whether the dongle re-appeared under
    keeberry's VID/PID and whether kcp round-trips over it. Every step is printed
    so the transcript is the diagnosis.
    """
    print("[1] kcp bus before anything:")
    _snapshot_kcp_bus()

    try:
        keyboard = find_kcp_interfaces(KEEBERRY_VID, KEEBERRY_PID)
    except Exception as exc:  # pragma: no cover - defensive
        print(f"  enumeration failed: {exc}")
        return 1
    if not keyboard:
        print(
            "\n[!] The keeberry keyboard (VID 0x1209/PID 0x0001) is not on the USB "
            "bus.\n    Connect it by cable to drive the pairing sequence, then re-run."
        )
        return 1

    with KcpDevice(keyboard[0]) as dev:
        print(f"\n[2] kcp over USB on {dev.match.describe()}")
        major, minor = dev.get_protocol(timeout_ms=args.timeout)
        print(f"    GET_PROTOCOL -> {major}.{minor}  (USB kcp OK)")
        s = dev.wls_get_state(timeout_ms=args.timeout)
        print(f"    WLS_GET_STATE -> mode={s['devs_label']} state={s['state_label']}")

        print("\n[3] switch output transport to 2.4 GHz")
        dev.wls_set_mode(DEVS_BY_NAME["2g4"], timeout_ms=args.timeout)
        print("    WLS_SET_MODE(2g4) OK")

        print("\n[4] pair the 2.4 GHz channel (this advertises keeberry VID/PID/strings)")
        dev.wls_pair(timeout_ms=args.timeout)
        print("    WLS_PAIR OK")

    # Give the dongle time to (potentially) re-enumerate under the new identity.
    print(f"\n[5] waiting {args.settle}s for the dongle to settle / re-enumerate...")
    time.sleep(args.settle)

    print("[6] kcp bus after pairing:")
    _snapshot_kcp_bus()

    print("\n[7] does kcp round-trip over every kcp interface now?")
    probed_any = False
    for m in find_kcp_interfaces():
        try:
            with KcpDevice(m) as dev:
                major, minor = dev.get_protocol(timeout_ms=args.timeout)
                probed_any = True
                print(f"    {m.describe()} -> GET_PROTOCOL {major}.{minor}  OK")
        except Exception as exc:
            print(f"    {m.describe()} -> {exc}")
    if not probed_any:
        print("    no kcp interface answered.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--vid",
        type=lambda v: int(v, 0),
        default=None,
        help="vendor id to match (default keeberry 0x1209)",
    )
    parser.add_argument(
        "--pid",
        type=lambda v: int(v, 0),
        default=None,
        help="product id to match (default keeberry 0x0001)",
    )
    parser.add_argument(
        "--any",
        action="store_true",
        help="match any VID/PID exposing the kcp usage (e.g. the Akko-VID dongle)",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=2000,
        help="per-request reply timeout in ms (default 2000; raise for a sleepy radio)",
    )

    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("enumerate", help="list every kcp (0xFF60/0x61) interface").set_defaults(
        func=cmd_enumerate
    )
    sub.add_parser("get-protocol", help="INFO 0x00 protocol-version ping").set_defaults(
        func=cmd_get_protocol
    )
    sub.add_parser("info", help="protocol + capabilities + device info").set_defaults(
        func=cmd_info
    )
    sub.add_parser("wls-state", help="WIRELESS 0x80 link snapshot").set_defaults(
        func=cmd_wls_state
    )
    p_mode = sub.add_parser("wls-mode", help="WIRELESS 0x81 select transport")
    p_mode.add_argument("mode", help="one of: " + ", ".join(DEVS_BY_NAME))
    p_mode.set_defaults(func=cmd_wls_mode)
    sub.add_parser("wls-pair", help="WIRELESS 0x82 (re)pair current transport").set_defaults(
        func=cmd_wls_pair
    )
    sub.add_parser("wls-battery", help="WIRELESS 0x85 battery level").set_defaults(
        func=cmd_wls_battery
    )
    sub.add_parser("config-info", help="CONFIG 0x42 persistence descriptor").set_defaults(
        func=cmd_config_info
    )
    sub.add_parser("config-save", help="CONFIG 0x40 persist live state to flash").set_defaults(
        func=cmd_config_save
    )
    p_diag = sub.add_parser(
        "diagnose", help="USB -> switch 2.4G -> pair -> re-enumerate, end to end"
    )
    p_diag.add_argument(
        "--settle",
        type=float,
        default=3.0,
        help="seconds to wait for the dongle to re-enumerate after pairing",
    )
    p_diag.set_defaults(func=cmd_diagnose)

    return parser


def main(argv: Optional[list[str]] = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return args.func(args)
    except KcpError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    except OSError as exc:
        print(f"error: HID I/O failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
