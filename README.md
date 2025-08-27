# Overview of the `network-initializer` library
The `network-initializer` library manages the setup, execution, and teardown of a simulated drone-based network for client-server communication in an unreliable environment. It integrates with `wg_internal` for core types and protocols, `common` for shared utilities, and multiple external drone crates for routing. Uses channels for inter-node and controller communication, with a focus on configurable topologies and failure simulation (e.g., packet drops via PDR).

Provides a `NetworkInitializer` for parsing configs, initializing nodes (drones, clients, servers), running threaded simulations, and handling events/commands.

## Features
- Config parsing and validation: TOML-based network topology with drones, clients, servers; checks for unique IDs, valid PDR (0.0-1.0), bidirectional connections, no self-loops, client/server constraints (e.g., clients connect to 1-2 drones, servers to ≥2).
- Node instantiation: Dynamically generates drones from external crates (e.g., `RustDoIt`, `DroneDrone`) via factory pattern; creates clients (`WebBrowser`, `ChatClient`) and servers (`TextServer`, `MediaServer`, `ChatServer`).
- Channel-based communication: Crossbeam channels for packets, drone commands/events, node commands/events.
- Stateful lifecycle: `Uninitialized` (config load) → `Initialized` (channel/node setup) → `Running` (threaded execution).
- Simulation control: Start spawns threads with barriers for synchronization; stop sends shutdown/crash commands and joins handles.
- Event handling: Receivers for drone/node events (e.g., `DroneEvent`, `NodeEvent`); getters for command senders, network view, and channels.
- Testing: Unit tests for parsing, validation (e.g., unidirectional errors, duplicates), initialization, getters, and event propagation.
- Error management: `ConfigError` variants for parsing/validation issues.

## Architecture
### Core Components
- **NetworkInitializer<State>**: Phantom-typed struct for lifecycle stages; holds channels (communication, commands, events), config, node instances, thread handles, and network view.
- **Config**: Struct for drones (ID, PDR, connections), clients (ID, drone connections), servers (ID, drone connections); parsed from TOML.
- **Channel**: Wrapper for crossbeam sender/receiver pairs, used for packets, commands, events.
- **Network**: From `common`, represents topology with nodes and adjacents.
- **Generate Drone**: Factory selector cycles through external drone implementations for instantiation.
- **Parser/Validate Traits**: Extend `Config` for TOML parsing and multi-step validation (IDs, PDR, connections).
- **Common Commands**: Via `DroneCommand` (e.g., `Crash`) and `NodeCommand` (e.g., `Shutdown`, neighbor add/remove).
### Details
#### Initialization
- `new(config_path)`: Parses/validates TOML; panics on failure.
- `initialize()`: Sets up channels, instantiates drones/clients/servers with receivers/neighbors, builds network view.
#### Running
- `start_simulation()`: Spawns threads for drones (run directly), clients/servers (run with barriers); moves instances to threads.
- Getters: `get_drones()`/`get_clients()`/`get_servers()` return maps with PDR/types and command senders; `get_network_view()` clones topology; event receivers for monitoring.
#### Teardown
- `stop_simulation()`: Drops packet senders, sends shutdown/crash commands, joins threads; logs termination status.