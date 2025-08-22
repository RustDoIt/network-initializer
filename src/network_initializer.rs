// TODO: togliere
#![allow(dead_code)]
#![allow(unused_variables)]
use crate::parser::{Parse, Validate};
use crate::utils::{generate_drones, Channel};
use common::network::Network;
use common::types::{NodeCommand, NodeEvent};
use common::Processor;
use crossbeam::channel::{Receiver, Sender};
use std::any::Any;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::mpsc::channel;
use std::thread::JoinHandle;
use wg_internal::config::Config;
use wg_internal::controller::{DroneCommand, DroneEvent};
use wg_internal::drone::Drone;
use wg_internal::network::NodeId;
use wg_internal::packet::Packet;
use client::web_browser::{self, WebBrowser};

pub struct Uninitialized;
pub struct Initialized;
pub struct Running;

pub struct NetworkInitializer<State = Uninitialized> {
    // node_id, sender to that node
    communications_channels: HashMap<NodeId, Channel<Packet>>,
    // each drone has his command receiver, controller needs senders to send commands
    drone_command_channels: HashMap<NodeId, Sender<DroneCommand>>,
    // each node has his command receiver, controller needs senders to send commands
    node_command_channels: HashMap<NodeId, Sender<Box<dyn Any>>>,
    // controller receives events from drones
    drone_event_channel: Option<Receiver<DroneEvent>>,
    // controller receives events from nodes
    node_event_channel: Option<Receiver<Box<dyn Any>>>,
    config: Config,
    // do not exists
    state: std::marker::PhantomData<State>,
    // TODO: create topology based on config
    network_view: Option<Network>,

    // these are needed to NetworkInitializer<Running> to run each node
    initialized_clients: Vec<Box<dyn Processor>>,
    initialized_servers: Vec<Box<dyn Processor>>,
    initialized_drones: Vec<Box<dyn Drone>>,

    // to keep track of threads and join them at the end
    node_handles: HashMap<NodeId, JoinHandle<()>>,
}

impl NetworkInitializer<Uninitialized> {
    pub fn new(config_path: &str) -> Self {
        let config = Config::parse_config(config_path).expect("Failed to parse config");
        if let Err(e) = config.validate_config() {
            panic!("Configuration validation failed: {}", e);
        }
        Self {
            communications_channels: HashMap::new(),
            drone_command_channels: HashMap::new(),
            node_command_channels: HashMap::new(),
            drone_event_channel: None,
            node_event_channel: None,
            config,
            // do not exists
            state: std::marker::PhantomData,
            network_view: None,
            initialized_clients: Vec::new(),
            initialized_servers: Vec::new(),
            initialized_drones: Vec::new(),
            node_handles: HashMap::new(),
        }
    }

    pub fn initialize(mut self) -> NetworkInitializer<Initialized> {
        self.initialize_drones();
        self.initialize_clients();
        self.initialize_servers();
        NetworkInitializer::<Initialized>::new(self)
    }

    fn initialize_drones(&mut self) {

        let mut drones_attributes = Vec::new();
        let event_channel = Channel::new();
        self.drone_event_channel = Some(event_channel.get_receiver());

        // first create all channels
        for d in self.config.drone.iter() {
            self.communications_channels.insert(d.id, Channel::new());
        }

        // then this
        for d in self.config.drone.iter() {
            let command_channel = Channel::new();
            let mut neighbours = HashMap::new();
            for id in d.connected_node_ids.iter() {
                if let Some(channel) = self.communications_channels.get(id) {
                    neighbours.insert(*id, channel.get_sender());
                }
            }
            // initializing receiver channel of the drone
            if let Some(packet_receiver) = self.communications_channels.get(&d.id){
                drones_attributes.push((d.id, command_channel.get_receiver(), packet_receiver.get_receiver(), neighbours, d.pdr));
            }
            
        }

        self.initialized_drones = generate_drones(event_channel.get_sender(), drones_attributes);
    }

    fn initialize_clients(&mut self) {
       unimplemented!()
    }

    fn initialize_servers(&mut self) {
        unimplemented!()
    }
}

impl NetworkInitializer<Initialized> {
    fn new(initializer: NetworkInitializer<Uninitialized>) -> Self {
        unimplemented!()
    }

    pub fn start_simulation(&mut self) -> NetworkInitializer<Running> {
        unimplemented!()
    }
}

impl NetworkInitializer<Running> {
    fn new(initializer: NetworkInitializer<Initialized>) -> Self {
        unimplemented!()
    }

    pub fn stop_simulation(&self) {
        unimplemented!()
    }

    pub fn get_drones(&self) -> HashMap<NodeId, (Sender<DroneCommand>, Receiver<DroneEvent>)> {
        unimplemented!()
    }

    pub fn get_clients(&self) -> HashMap<NodeId, (Sender<NodeCommand>, Receiver<NodeEvent>)> {
        unimplemented!()
    }

    pub fn get_servers(&self) -> HashMap<NodeId, (Sender<NodeCommand>, Receiver<NodeEvent>)> {
        unimplemented!()
    }

    fn get_network_view(&self) -> Network {
        unimplemented!()
    }
}
