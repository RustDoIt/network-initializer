// TODO: togliere
#![allow(dead_code)]
#![allow(unused_variables)]
use crate::parser::{Parse, Validate};
use crate::utils::{Channel, generate_drone};
use client::chat_client::ChatClient;
use client::web_browser::WebBrowser;
use common::Processor;
use common::network::Network;
use common::types::{Command, Event, NodeCommand, NodeType as CommonNodeType};
use crossbeam::channel::{Receiver, Sender};
use server::{ChatServer, MediaServer, TextServer};
use std::collections::HashMap;
use std::sync::{Arc, Barrier};
use std::thread::JoinHandle;
use wg_internal::config::Config;
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::drone::Drone;
use wg_internal::network::NodeId;
use wg_internal::packet::{NodeType, Packet};

pub struct Uninitialized;
pub struct Initialized;
pub struct Running;

pub struct NetworkInitializer<State = Uninitialized> {
    // node_id, sender to that node
    communications_channels: HashMap<NodeId, Channel<Packet>>,
    // each drone has his command receiver, controller needs senders to send commands
    drone_command_channels: HashMap<NodeId, Sender<DroneCommand>>,
    // each node has his command receiver, controller needs senders to send commands
    node_command_channels: HashMap<NodeId, (CommonNodeType, Sender<Box<dyn Command>>)>,
    // controller receives events from drones
    drone_event_channel: Channel<DroneEvent>,
    // controller receives events from nodes
    node_event_channel: Channel<Box<dyn Event>>,
    total_nodes: usize,
    pub(crate) config: Config,
    // do not exists
    state: std::marker::PhantomData<State>,

    network_view: Option<Network>,

    // these are needed to NetworkInitializer<Running> to run each node
    initialized_clients: HashMap<NodeId, Box<dyn Processor + Send>>,
    initialized_servers: HashMap<NodeId, Box<dyn Processor + Send>>,
    initialized_drones: HashMap<NodeId, Box<dyn Drone>>,

    // to keep track of threads and join them at the end
    node_handles: HashMap<NodeId, JoinHandle<()>>,
}

impl NetworkInitializer<Uninitialized> {
    /// # Panics
    /// Panics it cannot parse the config or the config is not a valid config
    #[must_use]
    pub fn new(config_path: &str) -> Self {
        let config = Config::parse_config(config_path).expect("Failed to parse config");
        config.validate_config().expect("Failed to validate config");
        Self {
            communications_channels: HashMap::new(),
            drone_command_channels: HashMap::new(),
            node_command_channels: HashMap::new(),
            drone_event_channel: Channel::new(),
            node_event_channel: Channel::new(),
            total_nodes: config.drone.len() + config.client.len() + config.server.len(),
            config,
            // do not exists
            state: std::marker::PhantomData,
            network_view: None,
            initialized_clients: HashMap::new(),
            initialized_servers: HashMap::new(),
            initialized_drones: HashMap::new(),
            node_handles: HashMap::new(),
        }
    }

    #[must_use]
    pub fn initialize(mut self) -> NetworkInitializer<Initialized> {
        self.initialize_channels();
        self.initialize_drones();
        self.initialize_clients();
        self.initialize_servers();
        self.inizialize_network_view();
        NetworkInitializer::<Initialized>::new(self)
    }

    fn initialize_drones(&mut self) {
        // then this
        for (i, d) in self.config.drone.iter().enumerate() {
            // initializing receiver channel of the drone
            if let Some(packet_receiver) = self.communications_channels.get(&d.id) {
                let command_channel = Channel::new();
                let mut neighbors = HashMap::new();
                for id in &d.connected_node_ids {
                    if let Some(channel) = self.communications_channels.get(id) {
                        neighbors.insert(*id, channel.get_sender());
                    }
                }

                self.drone_command_channels
                    .insert(d.id, command_channel.get_sender());
                self.initialized_drones.insert(
                    d.id,
                    generate_drone(
                        i,
                        &self.drone_event_channel.sender,
                        (
                            d.id,
                            command_channel.get_receiver(),
                            packet_receiver.get_receiver(),
                            neighbors,
                            d.pdr,
                        ),
                    ),
                );
            }
        }
    }

    fn initialize_clients(&mut self) {
        for (idx, c) in self.config.client.iter().enumerate() {
            // create neighbors
            if let Some(packet_channel) = self.communications_channels.get(&c.id) {
                let mut neighbors = HashMap::new();
                c.connected_drone_ids.iter().for_each(|id| {
                    if let Some(channel) = self.communications_channels.get(id) {
                        neighbors.insert(*id, channel.get_sender());
                    }
                });
                //create the channels
                let command_channel = Channel::new();
                #[allow(clippy::needless_late_init)]
                let client: Box<dyn Processor>;
                let node_type: CommonNodeType;
                // instantiate client
                if idx == 0 {
                    client = Box::new(WebBrowser::new(
                        c.id,
                        neighbors,
                        packet_channel.get_receiver(),
                        command_channel.get_receiver(),
                        self.node_event_channel.get_sender(),
                    ));
                    node_type = CommonNodeType::WebBrowser;
                } else {
                    client = Box::new(ChatClient::new(
                        c.id,
                        neighbors,
                        packet_channel.get_receiver(),
                        command_channel.get_receiver(),
                        self.node_event_channel.get_sender(),
                    ));
                    node_type = CommonNodeType::ChatClient;
                }

                // save the channels

                self.node_command_channels
                    .insert(c.id, (node_type, command_channel.get_sender()));

                // save the client
                self.initialized_clients.insert(c.id, client);
            }
        }
    }

    fn initialize_servers(&mut self) {
        for (i, s) in self.config.server.iter().enumerate() {
            if let Some(packet_channel) = self.communications_channels.get(&s.id) {
                let server: Box<dyn Processor>;
                let mut neighbors = HashMap::new();
                s.connected_drone_ids.iter().for_each(|id| {
                    if let Some(channel) = self.communications_channels.get(id) {
                        neighbors.insert(*id, channel.get_sender());
                    }
                });
                let node_type: CommonNodeType;

                let command_channel = Channel::new();

                match i % 3 {
                    0 => {
                        server = Box::new(TextServer::new(
                            s.id,
                            neighbors.clone(),
                            packet_channel.get_receiver(),
                            command_channel.get_receiver(),
                            self.node_event_channel.get_sender(),
                        ));
                        node_type = CommonNodeType::TextServer;
                    }
                    1 => {
                        server = Box::new(MediaServer::new(
                            s.id,
                            neighbors.clone(),
                            packet_channel.get_receiver(),
                            command_channel.get_receiver(),
                            self.node_event_channel.get_sender(),
                        ));
                        node_type = CommonNodeType::MediaServer;
                    }
                    2 => {
                        server = Box::new(ChatServer::new(
                            s.id,
                            neighbors.clone(),
                            packet_channel.get_receiver(),
                            command_channel.get_receiver(),
                            self.node_event_channel.get_sender(),
                        ));
                        node_type = CommonNodeType::ChatServer;
                    }
                    _ => unreachable!(),
                }

                self.node_command_channels
                    .insert(s.id, (node_type, command_channel.get_sender()));
                self.initialized_servers.insert(s.id, server);
            }
        }
    }

    fn inizialize_network_view(&mut self) {
        let mut network = Network::default();
        for d in &self.config.drone {
            network.add_node_controller_view(d.id, NodeType::Drone, &d.connected_node_ids);
        }
        for c in &self.config.client {
            network.add_node_controller_view(c.id, NodeType::Client, &c.connected_drone_ids);
        }

        for s in &self.config.server {
            network.add_node_controller_view(s.id, NodeType::Server, &s.connected_drone_ids);
        }
        self.network_view = Some(network);
    }

    fn initialize_channels(&mut self) {
        for d in &self.config.drone {
            self.communications_channels.insert(d.id, Channel::new());
        }
        for c in &self.config.client {
            self.communications_channels.insert(c.id, Channel::new());
        }
        for s in &self.config.server {
            self.communications_channels.insert(s.id, Channel::new());
        }
    }
}

impl NetworkInitializer<Initialized> {
    fn new(initializer: NetworkInitializer<Uninitialized>) -> Self {
        Self {
            communications_channels: initializer.communications_channels,
            drone_command_channels: initializer.drone_command_channels,
            node_command_channels: initializer.node_command_channels,
            drone_event_channel: initializer.drone_event_channel,
            node_event_channel: initializer.node_event_channel,
            total_nodes: initializer.total_nodes,
            config: initializer.config,
            state: std::marker::PhantomData,
            network_view: initializer.network_view,
            initialized_clients: initializer.initialized_clients,
            initialized_servers: initializer.initialized_servers,
            initialized_drones: initializer.initialized_drones,
            node_handles: HashMap::new(),
        }
    }

    #[must_use]
    pub fn start_simulation(mut self) -> NetworkInitializer<Running> {
        let barrier = Arc::new(Barrier::new(self.total_nodes - self.config.drone.len()));
        for (id, mut drone) in self.initialized_drones.drain() {
            let handle = std::thread::spawn(move || {
                drone.run();
            });
            self.node_handles.insert(id, handle);
        }
        for (id, mut client) in self.initialized_clients.drain() {
            let barrier = barrier.clone();
            let handle = std::thread::spawn(move || {
                client.run(barrier);
            });
            self.node_handles.insert(id, handle);
        }
        for (id, mut server) in self.initialized_servers.drain() {
            let barrier = barrier.clone();
            let handle = std::thread::spawn(move || {
                server.run(barrier);
            });
            self.node_handles.insert(id, handle);
        }
        NetworkInitializer::<Running>::new(self)
    }
}

impl NetworkInitializer<Running> {
    fn new(initializer: NetworkInitializer<Initialized>) -> Self {
        assert!(
            initializer.initialized_drones.is_empty(),
            "Drones should have been moved"
        );
        assert!(
            initializer.initialized_clients.is_empty(),
            "Clients should have been moved"
        );
        assert!(
            initializer.initialized_servers.is_empty(),
            "Servers should have been moved"
        );
        assert!(
            initializer.node_handles.len() == initializer.total_nodes,
            "All nodes should have been started"
        );
        assert!(
            initializer.network_view.is_some(),
            "Network should be initialized"
        );

        Self {
            communications_channels: initializer.communications_channels,
            drone_command_channels: initializer.drone_command_channels,
            node_command_channels: initializer.node_command_channels,
            drone_event_channel: initializer.drone_event_channel,
            node_event_channel: initializer.node_event_channel,
            total_nodes: initializer.total_nodes,
            config: initializer.config,
            state: std::marker::PhantomData,
            network_view: initializer.network_view,
            initialized_clients: initializer.initialized_clients,
            initialized_servers: initializer.initialized_servers,
            initialized_drones: initializer.initialized_drones,
            node_handles: initializer.node_handles,
        }
    }

    /// # Panics
    /// Panics if it cannot join handle
    pub fn stop_simulation(&mut self) {
        for (id, (node_type, channel)) in self.node_command_channels.drain() {
            if let Some(packet_sender) = self.communications_channels.remove(&id) {
                drop(packet_sender);
            }
            let _ = channel.send(Box::new(NodeCommand::Shutdown));
            match self.node_handles.remove(&id) {
                Some(handle) => match handle.join() {
                    Ok(()) => {
                        println!("Terminated a {node_type:?} thread successfully");
                    }
                    Err(e) => {
                        eprintln!("Failed to join a {node_type:?} thread: {e:?}");
                    }
                },
                None => {
                    eprintln!("No handle found for node {id}");
                }
            }
        }
        for (id, channel) in self.drone_command_channels.drain() {
            if let Some(packet_sender) = self.communications_channels.remove(&id) {
                drop(packet_sender);
            }
            let _ = channel.send(DroneCommand::Crash);
            match self.node_handles.remove(&id) {
                Some(handle) => match handle.join() {
                    Ok(()) => {
                        println!("Terminated a drone thread successfully");
                    }
                    Err(e) => {
                        eprintln!("Failed to join a drone thread: {e:?}");
                    }
                },
                None => {
                    eprintln!("No handle found for drone {id}");
                }
            }
        }
    }

    #[must_use]
    pub fn get_nodes_event_receiver(&self) -> Receiver<Box<dyn Event>> {
        self.node_event_channel.get_receiver()
    }

    #[must_use]
    pub fn get_drones_event_receiver(&self) -> Receiver<DroneEvent> {
        self.drone_event_channel.get_receiver()
    }

    #[must_use]
    pub fn get_drones(&self) -> HashMap<NodeId, (f32, Sender<DroneCommand>)> {
        let mut map = HashMap::new();
        for d in &self.config.drone {
            if let Some(channel) = self.drone_command_channels.get(&d.id) {
                map.insert(d.id, (d.pdr, channel.clone()));
            }
        }
        map
    }

    #[must_use]
    pub fn get_clients(&self) -> HashMap<NodeId, (CommonNodeType, Sender<Box<dyn Command>>)> {
        let mut map = HashMap::new();
        for c in &self.config.client {
            if let Some((node_type, channel)) = self.node_command_channels.get(&c.id) {
                map.insert(c.id, (*node_type, channel.clone()));
            }
        }
        map
    }

    #[must_use]
    pub fn get_servers(&self) -> HashMap<NodeId, (CommonNodeType, Sender<Box<dyn Command>>)> {
        let mut map = HashMap::new();
        for s in &self.config.server {
            if let Some((node_type, channel)) = self.node_command_channels.get(&s.id) {
                map.insert(s.id, (*node_type, channel.clone()));
            }
        }
        map
    }

    #[must_use]
    /// # Panics
    /// Panisce if not Initialized
    pub fn get_network_view(&self) -> Network {
        self.network_view.clone().expect("Network not Initialized")
    }

    #[must_use]
    pub fn get_comms_channels(&self) -> &HashMap<NodeId, Channel<Packet>> {
        &self.communications_channels
    }
}
