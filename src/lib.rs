mod errors;
pub mod network_initializer;
mod parser;
#[macro_use]
mod utils;

#[cfg(test)]
mod tests {

    type Simulation = (
        NetworkInitializer<Running>,
        HashMap<NodeId, (f32, Sender<DroneCommand>)>,
        HashMap<NodeId, (NodeType, Sender<Box<dyn Command>>)>,
        HashMap<NodeId, (NodeType, Sender<Box<dyn Command>>)>,
        Network, // HashMap<NodeId, Channel<Packet>>,
        Receiver<Box<dyn Event>>,
    );

    use std::collections::HashMap;
    use std::time::Duration;
    use crate::errors::ConfigError;
    use crate::network_initializer::NetworkInitializer;
    use crate::network_initializer::Running;
    use crate::network_initializer::Uninitialized;
    use crate::parser::Parse;
    use crate::parser::Validate;
    use common::network::Network;
    use common::types::{Event, WebRequest};
    // use crate::utils::Channel;
    use common::types::NodeCommand;
    use common::types::NodeEvent;
    use common::types::NodeType;
    use common::types::WebEvent;
    use common::types::{ChatCommand, Command, Message, WebCommand};
    use crossbeam::channel::{unbounded, Receiver};
    use crossbeam::channel::Sender;
    use wg_internal::config::Config;
    use wg_internal::controller::DroneCommand;
    use wg_internal::drone::Drone;
    use wg_internal::network::{NodeId, SourceRoutingHeader};
    use wg_internal::packet::{Packet, PacketType};
    use common::Processor;
    use common::types::NodeType::WebBrowser;
    // use wg_internal::packet::Packet;

    fn gen_simulation(path: &str) -> Simulation {
        let initializer = NetworkInitializer::<Uninitialized>::new(path)
            .initialize()
            .start_simulation();
        let clients = initializer.get_clients();
        let servers = initializer.get_servers();
        let drones = initializer.get_drones();
        let network = initializer.get_network_view();
        let event_recv = initializer.get_nodes_event_receiver();

        (initializer, drones, clients, servers, network, event_recv)
    }

    fn stop_simulation(sim: Simulation) {
        let (mut running, _drones, _clients, _servers, _network, _event) = sim;
        running.stop_simulation();
    }
    
    fn print_event(evt: Box<dyn Event>) {
        let evt = evt.into_any();
        if let Some(evt) = evt.downcast_ref::<NodeEvent>() {
            println!("NodeEvent: {:?}", evt);
        } else if let Some(evt) = evt.downcast_ref::<WebEvent>() {
            println!("WebEvent: {:?}", evt);
        } else {
            println!("Unknown event type");
        }
    }

    #[test]
    fn test_parse_config() {
        let config = Config::parse_config("./tests/correct_config.toml");
        assert!(config.is_ok());
    }

    #[test]
    fn test_validate_config() {
        let config = Config::parse_config("./tests/correct_config.toml").unwrap();
        let validation = config.validate_config();
        assert!(validation.is_ok());
    }

    #[test]
    fn test_unidirectional_error() {
        let config = Config::parse_config("./tests/unidirectional_error.toml").unwrap();
        let validation = config.validate_config();
        assert_eq!(validation, Err(ConfigError::UnidirectedConnection));
    }

    #[test]
    fn test_parsing_error() {
        let config = Config::parse_config("./tests/invalid_config.toml");
        assert!(config.is_err());
    }

    #[test]
    fn test_invalid_node_connection() {
        let config = Config::parse_config("./tests/invalid_node_connection1.toml").unwrap();
        let validation = config.validate_config();
        assert_eq!(
            validation,
            Err(ConfigError::InvalidNodeConnection(
                "Drone 3 cannot be connected to itself".to_string()
            ))
        );
    }

    #[test]
    fn test_network_initializer() {
        let net_init = NetworkInitializer::<Uninitialized>::new("./tests/correct_config.toml");
        let _net_init = net_init.initialize();

        println!("Initialized!");
    }

    #[test]
    fn test_getters_after_running() {
        let config_path = "./config/butterfly.toml";
        let mut running = NetworkInitializer::<Uninitialized>::new(config_path)
            .initialize()
            .start_simulation();

        // Check clients, servers, drones maps
        let drones = running.get_drones();
        let clients = running.get_clients();
        let servers = running.get_servers();

        assert!(!drones.is_empty(), "Drones should not be empty");
        assert!(!clients.is_empty(), "Clients should not be empty");
        assert!(!servers.is_empty(), "Servers should not be empty");

        // Drones may be optional depending on config
        // but if present, check channels are usable
        for (_, (_, tx)) in &drones {
            assert!(
                tx.send(wg_internal::controller::DroneCommand::Crash)
                    .is_ok()
            );
            // we can't fully check rx without running simulation events
        }

        for (_, (_, tx)) in clients {
            assert!(tx.send(Box::new(NodeCommand::Shutdown)).is_ok());
            // we can't fully check rx without running simulation events
        }

        for (_, (_, tx)) in servers {
            assert!(tx.send(Box::new(NodeCommand::Shutdown)).is_ok());
            // we can't fully check rx without running simulation events
        }
        running.stop_simulation();
    }

    #[test]
    fn test_simple_config() {
        let config_path = "./config/simple_config.toml";
        let (running_sim, drones, clients, servers, network, _event) = gen_simulation(config_path);
        assert_eq!(drones.len(), 2, "Drones should be 2");
        assert_eq!(clients.len(), 1, "Client should be 1");
        assert_eq!(servers.len(), 1, "Server should be 1");
        assert_eq!(network.nodes.len(), 4, "Nodes should be 4");
        assert_eq!(
            clients.get(&1).unwrap().0,
            NodeType::WebBrowser,
            "Client should be a WebBrowser"
        );
        assert_eq!(
            servers.get(&4).unwrap().0,
            NodeType::TextServer,
            "Server should be a TextServer"
        );
        assert_eq!(
            network
                .nodes
                .iter()
                .find(|n| n.id == 1)
                .unwrap()
                .get_adjacents(),
            &running_sim
                .config
                .client
                .iter()
                .find(|c| c.id == 1)
                .unwrap()
                .connected_drone_ids,
            "Adjacents of client 1 are not the expected"
        );
        assert_eq!(
            network
                .nodes
                .iter()
                .find(|n| n.id == 2)
                .unwrap()
                .get_adjacents(),
            &running_sim
                .config
                .drone
                .iter()
                .find(|c| c.id == 2)
                .unwrap()
                .connected_node_ids,
            "Adjacents of drone 2 are not the expected"
        );
        assert_eq!(
            network
                .nodes
                .iter()
                .find(|n| n.id == 3)
                .unwrap()
                .get_adjacents(),
            &running_sim
                .config
                .drone
                .iter()
                .find(|c| c.id == 3)
                .unwrap()
                .connected_node_ids,
            "Adjacents of drone 3 are not the expected"
        );
        assert_eq!(
            network
                .nodes
                .iter()
                .find(|n| n.id == 4)
                .unwrap()
                .get_adjacents(),
            &running_sim
                .config
                .server
                .iter()
                .find(|s| s.id == 4)
                .unwrap()
                .connected_drone_ids,
            "Adjacents of server 4 are not the expected"
        );
        let comms = running_sim.get_comms_channels();
        if let Some(ch) = comms.get(&4) {
            let _ = ch.send(Packet::new_flood_request(
                SourceRoutingHeader::empty_route(),
                1,
                wg_internal::packet::FloodRequest::new(1, 1),
            ));
        } else {
            panic!("Drone 2 channels not found");
        }
        
        std::thread::sleep(std::time::Duration::from_secs(2));

        stop_simulation((running_sim, drones, clients, servers, network, _event));
    }

    #[test]
    fn test_event_to_controller() {
        let config_path = "./config/simple_config.toml";
        let (running_sim, drones, clients, servers, network, event) = gen_simulation(config_path);
        let sender_client = &clients.get(&1).unwrap().1;
        let sender_server = &servers.get(&4).unwrap().1;


        for _ in 0..2 {
            let evt = event.recv().unwrap();
            let evt = evt.into_any();

            if let Some(evt) = evt.downcast_ref::<NodeEvent>() {
                assert!(matches!(*evt, NodeEvent::FloodStarted { .. }));
            } else {
                panic!("No FloodStarted event received, received other event: {evt:?}");
            }
        }

        for _ in 0..2 {
            let evt = event.recv().unwrap();
            let evt = evt.into_any();

            if let Some(evt) = evt.downcast_ref::<NodeEvent>() {
                assert!(matches!(*evt, NodeEvent::PacketSent { .. }));
            } else {
                panic!("No PacketSent event received, received other event: {evt:?}");
            }
        }
        let sender_server = &servers.get(&4).unwrap().1;
        let _result = sender_server.send(Box::new(WebCommand::AddTextFileFromPath(
            "./tests/non_existent.txt".to_string(),
        )));

        let event_1 = event.recv().unwrap();

        if let Ok(event_1) = event_1.into_any().downcast::<WebEvent>() {
            assert!(matches!(*event_1, WebEvent::FileOperationError { .. }));
        } else {
            panic!("Not TextFileAdded, other event");
        }

        let _result = sender_server.send(Box::new(WebCommand::AddTextFileFromPath(
            "./tests/test.txt".to_string(),
        )));

        let event_2 = event.recv().unwrap();
        let event_2 = event_2.into_any();

        if let Some(event_2) = event_2.downcast_ref::<WebEvent>() {
            assert!(matches!(*event_2, WebEvent::TextFileAdded { .. }));
        } else {
            panic!("Not TextFileAdded, other event");
        }


        // test event from client
        let _result = sender_client.send(Box::new(WebCommand::GetTextFilesList));
        let event_3 = event.recv().unwrap();
        let event_3 = event_3.into_any();
        if let Ok(event_3) = event_3.downcast::<WebEvent>() {
            assert!(
                matches!(*event_3, WebEvent::FilesLists { .. }),
                "Expected WebEvent::FilesLists but got {:?}",
                *event_3
            );
            if let WebEvent::FilesLists {
                notification_from,
                files_map,
            } = *event_3
            {
                assert!(notification_from == 1 && files_map.is_empty());
            } else {
                panic!("Expected WebEvent::FilesLists but got {:?}", *event_3);
            }
        } else {
            panic!("Not TextFileAdded, other event");
        }

        let _result = sender_client.send(Box::new(WebCommand::QueryTextFilesList));
        let event_4 = event.recv().unwrap();
        let event_4 = event_4.into_any();

        if let Ok(event_4) = event_4.downcast::<NodeEvent>() {
            // è giusto che panici, arriva FloodStarted
            assert!(matches!(*event_4, NodeEvent::PacketSent { .. }), "Expected NodeEvent::PacketSent but got {:?}", *event_4);
        } else {
            panic!("Not MessageSent, other event");
        }

        let event_5 = event.recv().unwrap();
        let event_5 = event_5.into_any();

        if let Ok(event_5) = event_5.downcast::<NodeEvent>() {
            // è giusto che panici, arriva FloodStarted
            assert!(matches!(*event_5, NodeEvent::MessageSent { .. }), "Expected NodeEvent::MessageSent but got {:?}", *event_5);
        } else {
            panic!("Not MessageSent, other event");
        }

        let event_6 = event.recv().unwrap();
        let event_6 = event_6.into_any();

        if let Ok(event_6) = event_6.downcast::<NodeEvent>() {
            // è giusto che panici, arriva FloodStarted
            assert!(matches!(*event_6, NodeEvent::PacketSent { .. }), "Expected NodeEvent::PacketSent but got {:?}", *event_6);
        } else {
            panic!("Not MessageSent, other event");
        }
        let event_7 = event.recv().unwrap();
        let event_7 = event_7.into_any();

        if let Ok(event_7) = event_7.downcast::<NodeEvent>() {
            assert!(
                matches!(*event_7, NodeEvent::MessageSent { .. }),
                "Expected MessageSent::FilesLists but got {:?}",
                *event_7
            );
        } else {
            panic!("Not TextFileAdded, other event");
        }

        let event_8 = event.recv().unwrap();
        let event_8 = event_8.into_any();
        if let Ok(event_8) = event_8.downcast::<NodeEvent>() {
            assert!(
                matches!(*event_8, NodeEvent::PacketSent { .. }),
                "Expected MessageSent::FilesLists but got {:?}",
                *event_8
            );
        } else {
            panic!("Not TextFileAdded, other event");
        }

        let event_9 = event.recv().unwrap();
        let event_9 = event_9.into_any();
        if let Ok(event_9) = event_9.downcast::<NodeEvent>() {
            assert!(
                matches!(*event_9, NodeEvent::MessageReceived { .. }),
                "Expected MessageSent::FilesLists but got {:?}",
                *event_9
            );
        } else {
            panic!("Not TextFileAdded, other event");
        }

        let event_10 = event.recv().unwrap();
        let event_10 = event_10.into_any();
        if let Ok(event_10) = event_10.downcast::<NodeEvent>() {
            assert!(
                matches!(*event_10, NodeEvent::ServerTypeQueried { .. }),
                "Expected NodeEvent::PacketSent but got {:?}",
                *event_10
            );
        } else {
            panic!("Not TextFileAdded, other event");
        }


        stop_simulation((running_sim, drones, clients, servers, network, event))
        
    }

}
