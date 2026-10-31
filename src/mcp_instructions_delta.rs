//! Hand translation of src/utils/mcpInstructionsDelta.ts (2.1.88).
//! Server-authored instructions are data, never a permission grant.
//! Vendor feature flags and telemetry have deliberately been omitted.
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedServer {
    pub name: String,
    pub connected: bool,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSideInstruction {
    pub server_name: String,
    pub block: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionAnnouncement {
    pub added_names: Vec<String>,
    pub removed_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpInstructionsDelta {
    pub added_names: Vec<String>,
    pub added_blocks: Vec<String>,
    pub removed_names: Vec<String>,
}

pub fn get_mcp_instructions_delta(
    clients: &[ConnectedServer],
    history: &[InstructionAnnouncement],
    client_side: &[ClientSideInstruction],
) -> Option<McpInstructionsDelta> {
    // 2.1.88 reconstructs the set from persisted delta attachments. Replay
    // additions and removals in message order, including reconnect cycles.
    let mut announced = HashSet::new();
    for event in history {
        for name in &event.added_names {
            announced.insert(name.to_owned());
        }
        for name in &event.removed_names {
            announced.remove(name);
        }
    }

    let connected: HashSet<&str> = clients
        .iter()
        .filter(|client| client.connected)
        .map(|client| client.name.as_str())
        .collect();
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut offsets: HashMap<String, usize> = HashMap::new();

    for client in clients.iter().filter(|client| client.connected) {
        if let Some(instructions) = client.instructions.as_deref().filter(|s| !s.is_empty()) {
            let block = format!("## {}\n{instructions}", client.name);
            if let Some(index) = offsets.get(&client.name).copied() {
                blocks[index].1 = block;
            } else {
                offsets.insert(client.name.clone(), blocks.len());
                blocks.push((client.name.clone(), block));
            }
        }
    }

    for instruction in client_side {
        if !connected.contains(instruction.server_name.as_str()) {
            continue;
        }
        if let Some(index) = offsets.get(&instruction.server_name).copied() {
            blocks[index].1.push_str("\n\n");
            blocks[index].1.push_str(&instruction.block);
        } else {
            offsets.insert(instruction.server_name.clone(), blocks.len());
            blocks.push((
                instruction.server_name.clone(),
                format!("## {}\n{}", instruction.server_name, instruction.block),
            ));
        }
    }

    let mut added: Vec<(String, String)> = blocks
        .into_iter()
        .filter(|(name, _)| !announced.contains(name))
        .collect();
    let mut removed: Vec<String> = announced
        .into_iter()
        .filter(|name| !connected.contains(name.as_str()))
        .collect();
    if added.is_empty() && removed.is_empty() {
        return None;
    }
    added.sort_by(|(name, _), (other, _)| name.cmp(other));
    removed.sort();
    let (added_names, added_blocks) = added.into_iter().unzip();
    Some(McpInstructionsDelta {
        added_names,
        added_blocks,
        removed_names: removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str, instructions: Option<&str>) -> ConnectedServer {
        ConnectedServer {
            name: name.into(),
            connected: true,
            instructions: instructions.map(str::to_owned),
        }
    }
    fn announcement(added: &[&str], removed: &[&str]) -> InstructionAnnouncement {
        InstructionAnnouncement {
            added_names: added.iter().map(|s| (*s).into()).collect(),
            removed_names: removed.iter().map(|s| (*s).into()).collect(),
        }
    }

    #[test]
    fn initial_connect_joins_both_instruction_channels_and_sorts() {
        let clients = [server("zeta", Some("Read only")), server("alpha", None)];
        let client_side = [
            ClientSideInstruction {
                server_name: "alpha".into(),
                block: "Use prompt A".into(),
            },
            ClientSideInstruction {
                server_name: "zeta".into(),
                block: "Client adds detail".into(),
            },
            ClientSideInstruction {
                server_name: "disconnected".into(),
                block: "Never inject".into(),
            },
        ];
        let result = get_mcp_instructions_delta(&clients, &[], &client_side).unwrap();
        assert_eq!(result.added_names, ["alpha", "zeta"]);
        assert_eq!(
            result.added_blocks,
            [
                "## alpha\nUse prompt A",
                "## zeta\nRead only\n\nClient adds detail"
            ]
        );
        assert!(result.removed_names.is_empty());
    }

    #[test]
    fn replay_announcements_detects_disconnection_and_reconnection() {
        let history = [announcement(&["a", "b"], &[]), announcement(&[], &["a"])];
        assert_eq!(
            get_mcp_instructions_delta(&[server("a", Some("new"))], &history, &[]).unwrap(),
            McpInstructionsDelta {
                added_names: vec!["a".into()],
                added_blocks: vec!["## a\nnew".into()],
                removed_names: vec!["b".into()],
            }
        );
        assert!(get_mcp_instructions_delta(
            &[server("a", Some("unchanged"))],
            &[announcement(&["a"], &[])],
            &[]
        )
        .is_none());
    }

    #[test]
    fn only_instruction_bearing_servers_are_announced() {
        let mut gone = server("gone", Some("unused"));
        gone.connected = false;
        assert!(get_mcp_instructions_delta(&[server("without", None), gone], &[], &[]).is_none());
    }
}
