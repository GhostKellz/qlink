# QLINK Architecture

QLINK is a **Rust-based, air‑gapped QR bridge** that lets a desktop machine use a webcam to talk to a **Keystone Pro 3** (and other air‑gapped hardware wallets) without ever exposing private keys or requiring USB connectivity.

It is designed to be:

- **Linux‑first** (Arch focused, but portable to other distros)
- **Local‑only** (binds to loopback, no cloud dependencies)
- **Modular** (daemon, optional UI, optional browser extension)
- **Wallet‑agnostic** over time (Keystone first, others later)

---

## High‑Level Overview

```text
+----------------------+            +---------------------+
|  Web3 dApps          |            |  Hardware Wallet    |
|  (MetaMask,          |            |  Keystone Pro 3     |
|   HashPack, XRP etc) |            |                     |
+----------+-----------+            +----------+----------+
           |                                   ^
           | browser wallet / extension        | QR display
           v                                   |
+----------+-----------+   local API   +-------+----------+
|  QLINK Browser Ext   +<------------->+   QLINK Daemon   |
|  (optional)          |   (127.0.0.1) |   (Rust)         |
+----------------------+               +----+------+------+
                                             |
                                             | V4L2 frames
                                             v
                                      +------+------+
                                      |   Webcam    |
                                      | (Facecam,   |
                                      |  Logitech)  |
                                      +-------------+
```

The **daemon** (`qlinkd`) is the core. It owns:

- Camera access
- QR decoding / encoding
- Keystone QR protocol understanding
- A small local API for tools (browser extension, CLIs, UI) to talk to

The **browser extension** and **UI** are thin clients on top.

---

## Components

### 1. QLINK Daemon (`qlinkd`)

The daemon is a long‑running Rust process that:

- Initializes configuration and logging
- Opens and manages the webcam via **V4L2**
- Continuously captures frames or performs one‑shot scans
- Decodes QR codes using a QR decoding library
- Interprets Keystone‑specific QR payloads
- Exposes a **local‑only API** (HTTP or WebSocket) on `127.0.0.1`

**Key responsibilities:**

- Provide `scan-once` and `scan-keystone` style operations
- Optionally render outgoing QR codes (e.g. signed payloads) and make them available for a UI to display
- Ensure no private keys or sensitive secrets ever leave the hardware wallet
- Enforce security boundaries (loopback only, optional local API token, minimal logging)

**Core internal modules (Rust):**

- `config` — load/validate `qlink.toml` / env vars
- `logging` — setup `tracing`
- `camera` — V4L2 access, device selection, frame capture
- `qr` — decoding and encoding routines
- `keystone` — message types / protocol helpers for Keystone flows
- `api` — HTTP / WebSocket handlers for local clients
- `security` — config validation, input validation, redaction of logs

---

### 2. Camera Subsystem

The camera layer abstracts Linux V4L2 devices:

- Enumerates available `/dev/video*` devices
- Selects the configured device (e.g. Facecam)
- Configures resolution, pixel format, and frame rate
- Streams frames to the QR decoder

Design goals:

- Prefer **low‑latency**, continuous capture when scanning
- Provide a **test mode** to dump a frame to an image file for debugging
- Allow tuning later for different webcam models

---

### 3. QR Engine

The QR engine is responsible for:

- Converting camera frames → grayscale / luminance data
- Locating and decoding QR codes in a frame
- Handling absence of QR codes gracefully
- Debouncing repeated scans (avoid spamming identical payloads)
- Encoding structured data back into QR images for outbound flows

It should be designed as a pure-ish library module, so:

- It can be unit‑tested with static image fixtures
- It doesn’t know about cameras, HTTP, or Keystone—just strings/payloads

---

### 4. Keystone Protocol Layer

The **Keystone layer** understands how Keystone Pro 3 uses QR codes:

- Wallet connect QR payloads
- Transaction signing requests
- Signed transaction responses

Internally, this looks like:

- `enum KeystoneMessage { Connect(..), SignTx(..), SignMessage(..), Unknown(..) }`
- Parsing from decoded QR strings into structured types
- Basic validation and classification of payloads
- Helper functions to map to chains (ETH, HBAR, XRP, etc.)

The daemon does **not** sign or store anything—it simply bridges:

1. dApp → wallet request (QR displayed in browser / extension)
2. wallet → dApp response (QR displayed on device, scanned by QLINK, forwarded back)

---

### 5. Local API (Bridge Surface)

The daemon exposes a **small local API surface** for frontends:

**Possible HTTP endpoints:**

- `GET /health`
  - Basic liveness + camera status
- `POST /scan-once`
  - Captures a frame, tries to decode a QR, returns payload or timeout
- `POST /scan-keystone`
  - Keystone‑aware flow: may capture multiple frames, apply debouncing
- `POST /show-qr`
  - Accepts a payload and generates a QR representation (for a UI to render)

This layer must:

- Bind only to **`127.0.0.1` by default**
- Optionally enforce a simple API token (configurable)
- Never expose cameras or APIs to external network interfaces

---

### 6. Browser Extension (Optional)

The extension is a **thin client** that talks to `qlinkd`:

- Runs in Chromium/Brave/Chrome/Firefox
- Intercepts or cooperates with dApp flows that use QR‑based wallet connect or signing
- Sends “scan now” requests to `qlinkd`
- Receives decoded payloads and feeds them into the wallet / Web3 provider

Example flows:

- MetaMask Keystone connect
- HashPack / Hedera wallet connect
- XRP Toolkit QR flows

The extension **never** sees private keys—it only works with:

- Connection requests
- Transaction payloads
- Signed transaction blobs

---

### 7. Optional Desktop UI

The UI (Tauri, egui, or similar) is:

- A convenience layer for power users
- Not required for headless usage

Responsibilities:

- Show a live camera preview
- Indicate when a QR code is detected and decoded
- Let the user select camera device and resolution
- Present status indicators for:
  - camera
  - API
  - extension connectivity
- Provide a “Keystone mode” preset (optimized scan parameters)

The UI should communicate with `qlinkd` over the same local API as the extension.

---

## Security Model

Core security assumptions:

- Private keys **always** live only on the hardware wallet.
- QLINK **never** stores or generates signing keys.
- All traffic:
  - stays on the local machine (`127.0.0.1`),
  - uses simple, auditable formats (JSON over HTTP/WebSocket),
  - is under user control.

Mitigations / practices:

- Loopback‑only binding by default.
- Optional local API token.
- No logging of full QR payloads by default (configurable redaction).
- Config validation on startup (fail fast with clear errors).
- Explicit boundaries between:
  - camera access,
  - QR decode,
  - protocol interpretation,
  - network API.

---

## Data Flow Examples

### 1. Connect Wallet (Keystone ↔ Browser)

1. dApp / wallet extension generates a **connect QR**.
2. User opens QLINK (or browser extension prompts them).
3. QLINK shows live preview and decodes the QR if needed (or extension can handle QR generation and just call `scan-keystone` for responses).
4. Keystone Pro 3 scans the connect QR on screen.
5. Keystone displays **response QR** with connection data.
6. QLINK captures and decodes the response QR.
7. QLINK sends decoded payload back to the browser extension via local API.
8. Extension finalizes wallet connect in the browser.

### 2. Transaction Signing

1. dApp requests transaction signature via wallet.
2. Wallet generates QR request (sign‑this‑tx).
3. Keystone scans QR from screen and signs locally.
4. Keystone displays **signed transaction QR**.
5. QLINK scans it, decodes it, and sends it back to the wallet / dApp extension.
6. Wallet submits signed tx to the network.

At no point are private keys or seed phrases within reach of QLINK.

---

## Deployment Model

Typical single‑user flow:

- Arch Linux desktop / workstation.
- User installs `qlinkd` binary (or via package manager in future).
- Optional:
  - systemd service to start `qlinkd` on boot.
  - QLINK browser extension installed in Chromium/Brave.
  - Desktop UI for manual control and debugging.

QLINK is intentionally **local‑only** and does not require:

- remote servers,
- cloud accounts,
- external APIs (besides whatever the dApp/wallet already talks to).

---

## Future Extensions

- Support for additional air‑gapped wallets (Passport, Ngrave, etc.).
- Profiles for different camera models and lighting conditions.
- “Headless mode” for server / kiosk setups.
- CLI tools for scripting (e.g. `qlink scan-once`).
- Integration with your broader Ghost / CKTech Web3 stack (Vex, Wraith, Prism, GhostHash).

---
