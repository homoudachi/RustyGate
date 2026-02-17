# Functional Description: RustyGate (BACnet to MQTT Gateway)

## 1. Project Goal
A high-performance, cross-platform (Windows/Linux) edge gateway to bridge BACnet/IP networks with MQTT brokers. It prioritizes asynchronous processing, data integrity, and network stability. The primary directive is to achieve **strict adherence to ASHRAE Standard 135** to ensure full interoperability and readiness for formal BTL (BACnet Testing Laboratories) certification as a BACnet Gateway (B-GW).

## 2. High-Level Architecture
The application uses a "Headless Core + UI Controller" model:
* **The Core (Daemon/Service):** Runs the Tokio runtime, manages UDP sockets (`bacnet-rs`), handles the polling/COV schedule, and maintains the MQTT connection (`rumqttc`). Capable of running entirely headless.
* **The UI (Dioxus):** A cross-platform visual frontend used for configuration, discovery, and monitoring.

## 3. BACnet Compliance & Interoperability
* **Standardization:** Designed to meet the requirements of the B-GW (BACnet Gateway) device profile.
* **Documentation:** The system will generate and expose a standard Protocol Implementation Conformance Statement (PICS) outlining all supported objects, services, and BACnet Interoperability Building Blocks (BIBBs).

## 4. Functional Modules
### A. Network Layer
* **Interface Selection:** Explicit binding to specific Network Interface Cards (NICs) to ensure traffic routes correctly across complex topologies, such as bridging natively across Layer 2 OpenVPN TAP interfaces.
* **BBMD Client:** Registers as a Foreign Device to remote BACnet Broadcast Management Devices, maintaining the connection via Time-To-Live (TTL) packets. 

### B. BACnet Engine
* **Discovery:** Supports Global (Broadcast) and Targeted (Unicast) `Who-Is` requests.
* **Acquisition:** * **Polling:** Fixed-interval reading of `Present_Value`.
    * **COV (Change of Value):** Subscription-based pushing of values based on thresholds, with automatic fallback to polling if unsupported by the end device.
* **Control:** Write commands require explicit user authorization per-point and utilize BACnet Priority Arrays to ensure safe equipment overriding.

### C. MQTT Bridge
* **Schema:** Publishes rich JSON payloads (value, status flags, reliability, timestamp) to structured topics: `bacnet/{gateway_id}/{device_id}/{object_type}/{object_instance}`.
* **Batching:** Groups messages during high-traffic bursts to prevent broker flooding.

### D. Persistence Layer
* **Database:** Utilizes a local SQLite database to store the Device Map, Point Configurations, and Audit Logs, ensuring state survives restarts and handling large point counts efficiently.

### E. Test Automation Driver
* **Simulation Engine:** Includes a dedicated testing API that allows the gateway to send targeted write commands to virtual BACnet simulators on the LAN, immediately polling the result to generate automated pass/fail latency and accuracy reports.
