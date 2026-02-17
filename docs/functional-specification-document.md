# Functional Specification Document (FSD)



## 1. Introduction
This document defines the technical requirements, architecture, and data flows for the RustyGate BACnet-to-MQTT edge gateway. The system is built in Rust, utilizing `dioxus` for the frontend UI, `tokio` for async runtime, `bacnet-rs` for protocol parsing, `rumqttc` for message brokering, and `sqlx` (SQLite) for persistence.

## 2. Compliance & Device Profile (ASHRAE 135)
To ensure full BACnet compliance and BTL certifiability, the gateway shall act as a "Virtual BACnet Device" mapping non-BACnet data (MQTT) to BACnet, and vice versa.
* **Device Profile:** BACnet Gateway (B-GW).
* **Required BIBBs (BACnet Interoperability Building Blocks):**
    * **Data Sharing:** DS-RP-A/B (ReadProperty), DS-RPM-A/B (ReadPropertyMultiple), DS-WP-A/B (WriteProperty), DS-COV-A/B (Change of Value).
    * **Device Management:** DM-DDB-A/B (Dynamic Device Binding / Who-Is/I-Am), DM-DOB-B (Dynamic Object Binding), DM-DCC-B (Device Communication Control).
    * **Network Management:** NM-FD-A (Foreign Device Registration).

## 3. System Architecture
* **Process Model:** A single compiled binary executable. 
* **Threading:** * Main Thread: Dioxus Webview/Event Loop.
    * Tokio Worker Threads: Handling UDP listeners, MQTT TCP streams, and SQLite I/O.
* **Inter-Task Communication:** Uses `tokio::sync::mpsc` channels to pass commands from the UI to the Network Engine, and `tokio::sync::broadcast` to push real-time point updates back to the UI.

## 4. Network & Integration Specifications

### 4.1 BACnet/IP Interface
* **Socket Binding:** The system shall present a list of available host network interfaces. The user must select the specific interface to bind the UDP port `47808`. 
* **BBMD Registration:** The system shall send a `Register-Foreign-Device` (Service Choice 0x05) message to the configured BBMD IP/Port, renewing at a frequency of `TTL - 30 seconds`.
* **APDU Handling:** Must gracefully handle segmentation and appropriately sized Maximum APDU Length Accepted parameters to prevent buffer overflows during large `ReadPropertyMultiple` requests.

### 4.2 MQTT Payload Specification
* **Publish QoS:** The system shall default to Quality of Service 1 (At least once) for point updates.
* **Read Payload (JSON Schema):**
    ```json
    {
      "value": 22.5,
      "status_flags": [false, false, false, false], 
      "reliability": 0, 
      "timestamp_ms": 1708156000000
    }
    ```
* **Write Command Payload (JSON Schema):**
    ```json
    {
      "write_value": 24.0,
      "priority": 8,
      "relinquish": false
    }
    ```



## 5. Database Schema (SQLite)
The system shall use a local SQLite database to persist state. 
* **Table: `gateways`** (Gateway configuration, Device Object ID, MQTT credentials, active NIC).
* **Table: `devices`** (Target devices mapping).
* **Table: `points`** (Object mapping, polling rates, COV subscription status).

### 4.3 Web Management Interface
*   **Technology:** Axum (Backend) + Dioxus Web (Frontend).
*   **Features:**
    *   NIC selection and IP binding.
    *   Real-time MQTT connection status.
    *   Live BACnet point monitor.
    *   Database configuration (SQLite path).

## 7. Automated Testing Framework
*   **Ghost Device Simulator:** A programmable BACnet/IP device capable of simulating:
    *   Analog Inputs/Outputs with configurable COV increments.
    *   Binary Objects with varying reliability flags.
    *   Multi-state objects.
*   **Test Runner:** A scriptable engine that controls the Ghost Device via MQTT/SSH to simulate network jitter, device timeouts, and protocol-level errors to ensure Gateway resilience.
