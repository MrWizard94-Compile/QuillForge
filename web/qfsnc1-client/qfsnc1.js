/**
 * Quillforge QFSNC1 browser client (reference).
 *
 * Wire: one WebSocket binary message = magic "QFSNC1\0\0" + u32 LE length + CBOR.
 * CBOR shapes match serde + ciborium for WireRequest / WireResponse in qf-sync.
 * Brand: WPAI — no Microsoft marks.
 */
(function (global) {
  "use strict";

  const MAGIC = new Uint8Array([0x51, 0x46, 0x53, 0x4e, 0x43, 0x31, 0x00, 0x00]); // QFSNC1\0\0

  // --- minimal CBOR (maps, text, uint, bytes, arrays, null, bool) ---

  function encodeCbor(value) {
    const out = [];
    function pushUint(major, n) {
      if (n < 24) out.push((major << 5) | n);
      else if (n < 0x100) {
        out.push((major << 5) | 24, n);
      } else if (n < 0x10000) {
        out.push((major << 5) | 25, (n >> 8) & 0xff, n & 0xff);
      } else if (n < 0x100000000) {
        out.push(
          (major << 5) | 26,
          (n >>> 24) & 0xff,
          (n >>> 16) & 0xff,
          (n >>> 8) & 0xff,
          n & 0xff
        );
      } else {
        throw new Error("uint too large for this encoder");
      }
    }
    function enc(v) {
      if (v === null || v === undefined) {
        out.push(0xf6);
        return;
      }
      if (typeof v === "boolean") {
        out.push(v ? 0xf5 : 0xf4);
        return;
      }
      if (typeof v === "number") {
        if (!Number.isInteger(v) || v < 0) throw new Error("only non-neg int supported");
        pushUint(0, v);
        return;
      }
      if (typeof v === "string") {
        const bytes = new TextEncoder().encode(v);
        pushUint(3, bytes.length);
        for (const b of bytes) out.push(b);
        return;
      }
      if (v instanceof Uint8Array) {
        pushUint(2, v.length);
        for (const b of v) out.push(b);
        return;
      }
      if (Array.isArray(v)) {
        pushUint(4, v.length);
        for (const item of v) enc(item);
        return;
      }
      if (typeof v === "object") {
        const keys = Object.keys(v);
        pushUint(5, keys.length);
        for (const k of keys) {
          enc(k);
          enc(v[k]);
        }
        return;
      }
      throw new Error("unsupported CBOR value: " + typeof v);
    }
    enc(value);
    return new Uint8Array(out);
  }

  function decodeCbor(bytes) {
    let i = 0;
    function take(n) {
      if (i + n > bytes.length) throw new Error("cbor truncated");
      const s = bytes.subarray(i, i + n);
      i += n;
      return s;
    }
    function readLen(ai) {
      if (ai < 24) return ai;
      if (ai === 24) return take(1)[0];
      if (ai === 25) {
        const b = take(2);
        return (b[0] << 8) | b[1];
      }
      if (ai === 26) {
        const b = take(4);
        return ((b[0] << 24) | (b[1] << 16) | (b[2] << 8) | b[3]) >>> 0;
      }
      throw new Error("cbor length form not supported");
    }
    function dec() {
      const b = take(1)[0];
      const major = b >> 5;
      const ai = b & 0x1f;
      if (major === 0) return readLen(ai);
      if (major === 1) return -1 - readLen(ai);
      if (major === 2) return take(readLen(ai));
      if (major === 3) return new TextDecoder().decode(take(readLen(ai)));
      if (major === 4) {
        const n = readLen(ai);
        const arr = [];
        for (let k = 0; k < n; k++) arr.push(dec());
        return arr;
      }
      if (major === 5) {
        const n = readLen(ai);
        const obj = {};
        for (let k = 0; k < n; k++) {
          const key = dec();
          obj[key] = dec();
        }
        return obj;
      }
      if (major === 7) {
        if (ai === 20) return false;
        if (ai === 21) return true;
        if (ai === 22) return null;
        throw new Error("simple value " + ai);
      }
      throw new Error("cbor major " + major);
    }
    return dec();
  }

  // --- UUID helpers (16-byte big-endian style as used by uuid crate in CBOR) ---

  function uuidToBytes(uuidStr) {
    const hex = uuidStr.replace(/-/g, "");
    if (hex.length !== 32) throw new Error("bad uuid");
    const out = new Uint8Array(16);
    for (let i = 0; i < 16; i++) out[i] = parseInt(hex.substr(i * 2, 2), 16);
    return out;
  }

  function bytesToUuid(bytes) {
    const hex = [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
    return (
      hex.slice(0, 8) +
      "-" +
      hex.slice(8, 12) +
      "-" +
      hex.slice(12, 16) +
      "-" +
      hex.slice(16, 20) +
      "-" +
      hex.slice(20)
    );
  }

  function randomUuidBytes() {
    const b = new Uint8Array(16);
    crypto.getRandomValues(b);
    // RFC 4122 version 4 nibble
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    return b;
  }

  function zeroHash() {
    return new Uint8Array(32);
  }

  // --- QFSNC1 framing ---

  function encodeFrame(cborObj) {
    const payload = encodeCbor(cborObj);
    const frame = new Uint8Array(8 + 4 + payload.length);
    frame.set(MAGIC, 0);
    const len = payload.length;
    frame[8] = len & 0xff;
    frame[9] = (len >>> 8) & 0xff;
    frame[10] = (len >>> 16) & 0xff;
    frame[11] = (len >>> 24) & 0xff;
    frame.set(payload, 12);
    return frame;
  }

  function decodeFrame(buf) {
    const u8 = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
    if (u8.length < 12) throw new Error("frame too short");
    for (let i = 0; i < 8; i++) {
      if (u8[i] !== MAGIC[i]) throw new Error("bad QFSNC1 magic");
    }
    const len = u8[8] | (u8[9] << 8) | (u8[10] << 16) | (u8[11] << 24);
    if (len <= 0 || 12 + len > u8.length) throw new Error("bad frame length " + len);
    return decodeCbor(u8.subarray(12, 12 + len));
  }

  // --- high-level messages ---

  function helloMsg(clientId, sessionId, documentId, haveSeq) {
    return {
      type: "hello",
      client_id: clientId,
      session_id: sessionId,
      document_id: documentId,
      have_seq: haveSeq >>> 0,
    };
  }

  function pullMsg(token, haveSeq) {
    return { type: "pull", token: token, have_seq: haveSeq >>> 0 };
  }

  function pushNoopMsg(token, seq, note) {
    // Operation::Noop { reason: Other(note) } → {"Noop":{"reason":{"Other":note}}}
    return {
      type: "push",
      token: token,
      first_seq: seq >>> 0,
      last_seq: seq >>> 0,
      ops: [{ Noop: { reason: { Other: note || "browser" } } }],
      crdt: null,
      payload_hash: zeroHash(),
    };
  }

  /**
   * One-shot request/response over a new WebSocket (matches Rust client pattern).
   * @param {string} url ws://host:port/qfsnc1
   * @param {object} requestObj WireRequest-shaped object
   * @returns {Promise<object>}
   */
  function roundTrip(url, requestObj) {
    return new Promise((resolve, reject) => {
      let settled = false;
      const ws = new WebSocket(url);
      ws.binaryType = "arraybuffer";
      const timer = setTimeout(() => {
        if (!settled) {
          settled = true;
          try {
            ws.close();
          } catch (_) {}
          reject(new Error("timeout"));
        }
      }, 8000);
      ws.onopen = () => {
        try {
          ws.send(encodeFrame(requestObj));
        } catch (e) {
          if (!settled) {
            settled = true;
            clearTimeout(timer);
            reject(e);
          }
        }
      };
      ws.onmessage = (ev) => {
        if (settled) return;
        try {
          const resp = decodeFrame(ev.data);
          settled = true;
          clearTimeout(timer);
          ws.close();
          resolve(resp);
        } catch (e) {
          settled = true;
          clearTimeout(timer);
          reject(e);
        }
      };
      ws.onerror = () => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          reject(new Error("websocket error"));
        }
      };
      ws.onclose = () => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          reject(new Error("websocket closed before response"));
        }
      };
    });
  }

  async function demoFlow(url, log) {
    const logFn = log || console.log;
    const clientId = randomUuidBytes();
    const sessionId = randomUuidBytes();
    const documentId = randomUuidBytes();
    logFn("hello → " + url);
    const ack = await roundTrip(url, helloMsg(clientId, sessionId, documentId, 0));
    if (ack.type !== "hello_ack" && !ack.token) {
      // serde may use type hello_ack for HelloAck variant
      logFn("raw hello response: " + JSON.stringify(ack, replacer));
    }
    const token = ack.token;
    const head = ack.server_head_seq ?? 0;
    logFn("hello_ack token=" + bytesToUuid(token) + " head=" + head);

    const push = await roundTrip(url, pushNoopMsg(token, head + 1, "browser-demo"));
    logFn("push_ack accepted_head_seq=" + (push.accepted_head_seq ?? JSON.stringify(push, replacer)));

    const pull = await roundTrip(url, pullMsg(token, 0));
    logFn(
      "pull server_head_seq=" +
        (pull.server_head_seq ?? "?") +
        " ops=" +
        (Array.isArray(pull.ops) ? pull.ops.length : "?")
    );
    return { ack, push, pull };
  }

  function replacer(_k, v) {
    if (v instanceof Uint8Array) return bytesToUuid(v.length === 16 ? v : v);
    return v;
  }

  // Golden self-check: fixed IDs match Rust ciborium encoding (see qf-sync wire tests).
  function selfTest() {
    const hello = helloMsg(
      uuidToBytes("00000000-0000-0000-0000-000000000001"),
      uuidToBytes("00000000-0000-0000-0000-000000000002"),
      uuidToBytes("00000000-0000-0000-0000-000000000003"),
      0
    );
    const hex = [...encodeCbor(hello)].map((b) => b.toString(16).padStart(2, "0")).join("");
    const expected =
      "a564747970656568656c6c6f69636c69656e745f696450000000000000000000000000000000016a73657373696f6e5f696450000000000000000000000000000000026b646f63756d656e745f6964500000000000000000000000000000000368686176655f73657100";
    if (hex !== expected) {
      throw new Error("hello CBOR golden mismatch\n got " + hex + "\n want " + expected);
    }
    return true;
  }

  global.Qfsnc1 = {
    MAGIC,
    encodeCbor,
    decodeCbor,
    encodeFrame,
    decodeFrame,
    uuidToBytes,
    bytesToUuid,
    randomUuidBytes,
    helloMsg,
    pullMsg,
    pushNoopMsg,
    roundTrip,
    demoFlow,
    selfTest,
  };
})(typeof window !== "undefined" ? window : globalThis);
