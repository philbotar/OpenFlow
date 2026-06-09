use engine::CallableAgent;
use std::io;

pub trait AgentStore: Send + Sync {
    fn load(&self) -> io::Result<Vec<CallableAgent>>;
    fn save(&self, agents: &[CallableAgent]) -> io::Result<()>;
}
