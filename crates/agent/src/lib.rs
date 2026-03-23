pub mod claude;
pub mod definition;
pub mod pty;
pub mod traits;

pub use definition::{
    load_agent_definition, load_agent_definitions, to_claude_agents_json, validate_definitions,
    AgentDefinition, AgentDefinitionError, ClaudeAgentArgs, TemplateContext,
};
pub use traits::{Agent, AgentConfig, AgentError, AgentHandle, AgentOutput};
