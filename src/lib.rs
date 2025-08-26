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

    use crate::errors::ConfigError;
    use crate::network_initializer::NetworkInitializer;
    use crate::network_initializer::Running;
    use crate::network_initializer::Uninitialized;
    use crate::parser::Parse;
    use crate::parser::Validate;
    use common::network::Network;
    use common::types::ChatEvent;
    use common::types::Event;
    // use crate::utils::Channel;
    use common::types::NodeCommand;
    use common::types::NodeEvent;
    use common::types::NodeType;
    use common::types::WebEvent;
    use common::types::{ChatCommand, Command, Message, WebCommand};
    use crossbeam::channel::Receiver;
    use crossbeam::channel::Sender;
    use wg_internal::config::Config;
    use wg_internal::controller::DroneCommand;
    use wg_internal::network::NodeId;
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

        stop_simulation((running_sim, drones, clients, servers, network, _event));
    }

    #[test]
    fn test_event_to_controller() {
        let config_path = "./config/simple_config.toml";
        let (running_sim, drones, clients, servers, network, event) = gen_simulation(config_path);

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

        if let Ok(event_2) = event_2.into_any().downcast::<WebEvent>() {
            assert!(matches!(*event_2, WebEvent::TextFileAdded { .. }));
        } else {
            panic!("Not TextFileAdded, other event");
        }

        stop_simulation((running_sim, drones, clients, servers, network, event))
    }

    #[test]
    fn test_query_text_files_list() {
        let config_path = "./config/simple_config.toml";
        let (running_sim, drones, clients, servers, network, event) = gen_simulation(config_path);

        let sender_server = &servers.get(&4).unwrap().1;
        let _result = sender_server.send(Box::new(WebCommand::AddTextFileFromPath(
            "./tests/non_existent.txt".to_string(),
        )));

        let event_1 = event.recv().unwrap();

        let _result = sender_server.send(Box::new(WebCommand::AddTextFileFromPath(
            "./tests/test.txt".to_string(),
        )));

        let event_2 = event.recv().unwrap();

        let sender_client = &clients.get(&1).unwrap().1;
        let _result = sender_client.send(Box::new(WebCommand::QueryTextFilesList));
        let event_3 = event.recv().unwrap();
        if let Ok(event_3) = event_3.into_any().downcast::<NodeEvent>() {
            assert!(matches!(*event_2, WebEvent::TextFileAdded { .. }));
        } else {
            panic!("Not TextFileAdded, other event");
        }

        // TODO: flood network discovery

        std::thread::sleep(std::time::Duration::from_secs(3));

        let _result = sender_client.send(Box::new(WebCommand::GetTextFilesList));

        std::thread::sleep(std::time::Duration::from_secs(3));

        stop_simulation((running_sim, drones, clients, servers, network, event));
    }

    #[test]
    fn client_chatserver() {
        let config_path = "./config/simple_chat_config.toml";
        let (running_sim, drones, clients, servers, network, event) = gen_simulation(config_path);

        let sender_client_1 = &clients.get(&2).unwrap().1;
        let sender_client_2 = &clients.get(&8).unwrap().1;
        let sender_server = &servers.get(&6).unwrap().1;

        let _result = sender_client_1.send(Box::new(ChatCommand::RegisterToServer(6)));
        let event_1 = event.recv().unwrap();

        if let Ok(event_1) = event_1.into_any().downcast::<ChatEvent>() {
            assert!(matches!(*event_1, ChatEvent::RegistrationSucceeded { notification_from:6, to:2 }));
        } else {
            panic!("Client registration not successful");
        }

        let _result = sender_client_2.send(Box::new(ChatCommand::RegisterToServer(6)));
        let event_2 = event.recv().unwrap();

        if let Ok(event_2) = event_2.into_any().downcast::<ChatEvent>() {
            assert!(matches!(*event_2, ChatEvent::RegistrationSucceeded { notification_from:6, to:3 }));
        } else {
             panic!("Client registration not successful");
        }

        let event_3 = event.recv().unwrap();
        if let Ok(event_3) = event_3.into_any().downcast::<ChatEvent>() {
            assert!(matches!(*event_3, ChatEvent::RegisteredClients { notification_from: 6, list } if list.contains(&2) && list.contains(&8)));
        } else {
            panic!("Not GetRegisteredClients, other event");
        }

        // let _result = sender_client_1.send(Box::new(ChatCommand::GetRegisteredClients)); // 2, 3
        // let _result = sender_client_2.send(Box::new(ChatCommand::GetRegisteredClients)); // 2, 3

        // let message = Message::new(2, 3, "ciao 3, sono 2".to_string());
        // let _result = sender_client_1.send(Box::new(ChatCommand::SendMessage(message))); // esegue ma non manda per topologia mancante
        // let message = Message::new(2, 3, "ciao 2, messaggio ricevuto".to_string());
        // let _result = sender_client_1.send(Box::new(ChatCommand::SendMessage(message))); // esegue ma non manda per topologia mancante

        stop_simulation((running_sim, drones, clients, servers, network, event));
    }

}
