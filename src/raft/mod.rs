use serde::{Serialize, Deserialize};
use std::time::{Duration, Instant};
use rand::Rng;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RaftState {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: u64,
    pub command: Command,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Command {
    Set(String, String),
    Delete(String),
    Keys,
    Flush,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteArgs {
    pub term: u64,
    pub candidate_id: u64,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteReply {
    pub term: u64,
    pub vote_granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesArgs {
    pub term: u64,
    pub leader_id: u64,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesReply {
    pub term: u64,
    pub success: bool,
}

pub struct RaftNode {
    pub id: u64,
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub log: Vec<LogEntry>,
    pub state: RaftState,
    pub commit_index: u64,
    pub last_applied: u64,
    
    pub next_index: Vec<u64>,
    pub match_index: Vec<u64>,

    pub last_heartbeat: Instant,
    pub election_timeout: Duration,
}

impl RaftNode {
    pub fn new(id: u64, peer_count: usize) -> Self {
        let mut rng = rand::thread_rng();
        let timeout = Duration::from_millis(rng.gen_range(1000..2000));
        
        Self {
            id,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            state: RaftState::Follower,
            commit_index: 0,
            last_applied: 0,
            next_index: vec![1; peer_count],
            match_index: vec![0; peer_count],
            last_heartbeat: Instant::now(),
            election_timeout: timeout,
        }
    }

    pub fn restore(&mut self, term: u64, voted_for: Option<u64>, log: Vec<LogEntry>) {
        self.current_term = term;
        self.voted_for = voted_for;
        self.log = log;
    }

    pub fn handle_request_vote(&mut self, args: RequestVoteArgs) -> RequestVoteReply {
        if args.term < self.current_term {
            return RequestVoteReply { term: self.current_term, vote_granted: false };
        }
        if args.term > self.current_term {
            self.current_term = args.term;
            self.state = RaftState::Follower;
            self.voted_for = None;
        }
        let can_vote = self.voted_for.is_none() || self.voted_for == Some(args.candidate_id);
        let last_log_index = self.log.len() as u64;
        let last_log_term = self.log.last().map_or(0, |e| e.term);
        let log_is_uptodate = args.last_log_term > last_log_term || 
            (args.last_log_term == last_log_term && args.last_log_index >= last_log_index);

        if can_vote && log_is_uptodate {
            self.voted_for = Some(args.candidate_id);
            self.last_heartbeat = Instant::now();
            RequestVoteReply { term: self.current_term, vote_granted: true }
        } else {
            RequestVoteReply { term: self.current_term, vote_granted: false }
        }
    }

    pub fn handle_append_entries(&mut self, args: AppendEntriesArgs) -> AppendEntriesReply {
        self.last_heartbeat = Instant::now();
        if args.term < self.current_term {
            return AppendEntriesReply { term: self.current_term, success: false };
        }
        if args.term > self.current_term {
            self.current_term = args.term;
            self.state = RaftState::Follower;
        }
        if args.prev_log_index > 0 {
            let index = (args.prev_log_index - 1) as usize;
            if index >= self.log.len() || self.log[index].term != args.prev_log_term {
                return AppendEntriesReply { term: self.current_term, success: false };
            }
        }
        let mut entry_idx = args.prev_log_index as usize;
        for new_entry in args.entries {
            if entry_idx < self.log.len() {
                if self.log[entry_idx].term != new_entry.term {
                    self.log.truncate(entry_idx);
                    self.log.push(new_entry);
                }
            } else {
                self.log.push(new_entry);
            }
            entry_idx += 1;
        }
        if args.leader_commit > self.commit_index {
            self.commit_index = std::cmp::min(args.leader_commit, self.log.len() as u64);
        }
        AppendEntriesReply { term: self.current_term, success: true }
    }

    pub fn tick(&mut self, peer_count: usize) -> (Option<RequestVoteArgs>, Vec<(u64, AppendEntriesArgs)>) {
        match self.state {
            RaftState::Follower | RaftState::Candidate => {
                if self.last_heartbeat.elapsed() >= self.election_timeout {
                    (self.start_election(), vec![])
                } else {
                    (None, vec![])
                }
            }
            RaftState::Leader => {
                if self.last_heartbeat.elapsed() >= Duration::from_millis(10) {
                    self.last_heartbeat = Instant::now();
                    let mut heartbeats = vec![];
                    for peer_id in 0..peer_count as u64 {
                        if peer_id == self.id { continue; }
                        heartbeats.push((peer_id, self.make_append_entries(peer_id)));
                    }
                    (None, heartbeats)
                } else {
                    (None, vec![])
                }
            }
        }
    }

    fn make_append_entries(&self, peer_id: u64) -> AppendEntriesArgs {
        let next_idx = self.next_index[peer_id as usize];
        let entries = if self.log.len() as u64 >= next_idx {
            self.log[(next_idx as usize - 1)..].to_vec()
        } else {
            vec![]
        };
        let prev_log_index = next_idx - 1;
        let prev_log_term = if prev_log_index > 0 {
            self.log[prev_log_index as usize - 1].term
        } else {
            0
        };
        AppendEntriesArgs {
            term: self.current_term,
            leader_id: self.id,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit: self.commit_index,
        }
    }

    pub fn handle_vote_reply(&mut self, reply: RequestVoteReply, peer_count: usize, votes: &mut usize) {
        if reply.term > self.current_term {
            self.current_term = reply.term;
            self.state = RaftState::Follower;
            return;
        }
        if self.state == RaftState::Candidate && reply.vote_granted {
            *votes += 1;
            if *votes > peer_count / 2 {
                self.state = RaftState::Leader;
                for i in 0..self.next_index.len() {
                    self.next_index[i] = self.log.len() as u64 + 1;
                    self.match_index[i] = 0;
                }
            }
        }
    }

    fn start_election(&mut self) -> Option<RequestVoteArgs> {
        self.state = RaftState::Candidate;
        self.current_term += 1;
        self.voted_for = Some(self.id);
        self.last_heartbeat = Instant::now();
        let mut rng = rand::thread_rng();
        self.election_timeout = Duration::from_millis(rng.gen_range(1000..2000));
        let last_log_index = self.log.len() as u64;
        let last_log_term = self.log.last().map_or(0, |e| e.term);
        Some(RequestVoteArgs {
            term: self.current_term,
            candidate_id: self.id,
            last_log_index,
            last_log_term,
        })
    }

    pub fn get_unapplied_commands(&mut self) -> Vec<Command> {
        let mut commands = vec![];
        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            let entry = &self.log[self.last_applied as usize - 1];
            commands.push(entry.command.clone());
        }
        commands
    }

    pub fn handle_append_entries_reply(&mut self, peer_id: u64, reply: AppendEntriesReply) {
        if reply.term > self.current_term {
            self.current_term = reply.term;
            self.state = RaftState::Follower;
            return;
        }
        if self.state == RaftState::Leader {
            if reply.success {
                self.match_index[peer_id as usize] = self.log.len() as u64;
                self.next_index[peer_id as usize] = self.match_index[peer_id as usize] + 1;
                self.update_commit_index();
            } else if self.next_index[peer_id as usize] > 1 {
                self.next_index[peer_id as usize] -= 1;
            }
        }
    }

    fn update_commit_index(&mut self) {
        let mut match_indices = self.match_index.clone();
        match_indices[self.id as usize] = self.log.len() as u64;
        match_indices.sort();
        let majority_index = match_indices[match_indices.len() / 2];
        if majority_index > self.commit_index {
            if majority_index > 0 && self.log[majority_index as usize - 1].term == self.current_term {
                self.commit_index = majority_index;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_initial_state() {
        let node = RaftNode::new(0, 3);
        assert_eq!(node.id, 0);
        assert_eq!(node.current_term, 0);
        assert_eq!(node.state, RaftState::Follower);
    }

    #[test]
    fn test_request_vote_higher_term() {
        let mut node = RaftNode::new(0, 3);
        node.current_term = 1;

        let args = RequestVoteArgs {
            term: 2,
            candidate_id: 1,
            last_log_index: 0,
            last_log_term: 0,
        };

        let reply = node.handle_request_vote(args);
        assert!(reply.vote_granted);
        assert_eq!(node.current_term, 2);
        assert_eq!(node.voted_for, Some(1));
    }

    #[test]
    fn test_request_vote_log_up_to_date() {
        let mut node = RaftNode::new(0, 3);
        node.current_term = 1;
        node.log.push(LogEntry { term: 1, command: Command::Set("k".into(), "v".into()) });

        // Candidate has lower log index
        let args = RequestVoteArgs {
            term: 2,
            candidate_id: 1,
            last_log_index: 0,
            last_log_term: 0,
        };

        let reply = node.handle_request_vote(args);
        assert!(!reply.vote_granted);
    }

    #[test]
    fn test_append_entries_heartbeat() {
        let mut node = RaftNode::new(0, 3);
        let before = node.last_heartbeat;
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let args = AppendEntriesArgs {
            term: 1,
            leader_id: 1,
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let reply = node.handle_append_entries(args);
        assert!(reply.success);
        assert!(node.last_heartbeat > before);
    }

    #[test]
    fn test_leader_election() {
        let mut node = RaftNode::new(0, 3);
        let vote_args = node.start_election().unwrap();
        
        assert_eq!(node.state, RaftState::Candidate);
        assert_eq!(node.current_term, 1);
        assert_eq!(vote_args.term, 1);

        let reply = RequestVoteReply { term: 1, vote_granted: true };
        let mut votes = 1; // Already voted for self
        node.handle_vote_reply(reply, 3, &mut votes);
        
        assert_eq!(node.state, RaftState::Leader);
    }
}
