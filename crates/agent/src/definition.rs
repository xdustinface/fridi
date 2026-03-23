use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentDefinitionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub name: String,
    pub description: String,
    pub prompt: String,
    #[serde(default)]
    pub permissions: Option<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub spawnable_roles: Vec<String>,
    #[serde(default)]
    pub default_args: Vec<String>,
}

/// Template context for interpolating variables in agent prompts
pub struct TemplateContext {
    pub repo: String,
    pub session_id: String,
    pub session_dir: String,
    pub mcp_socket: String,
}

/// CLI arguments for spawning an agent via Claude Code
pub struct ClaudeAgentArgs {
    pub agents_json: String,
    pub agent_name: String,
    pub permission_mode: Option<String>,
    pub allowed_tools: Vec<String>,
    pub mcp_config: Option<String>,
    pub session_id: Option<String>,
    pub extra_args: Vec<String>,
}

/// Load a single agent definition from a YAML file
pub fn load_agent_definition(path: &Path) -> Result<AgentDefinition, AgentDefinitionError> {
    let content = std::fs::read_to_string(path)?;
    let def: AgentDefinition = serde_yaml::from_str(&content)?;
    Ok(def)
}

/// Load all agent definitions from a directory (`.yaml` and `.yml` files)
pub fn load_agent_definitions(
    dir: &Path,
) -> Result<Vec<AgentDefinition>, AgentDefinitionError> {
    let mut defs = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == "yaml" || ext == "yml" {
                defs.push(load_agent_definition(&path)?);
            }
        }
    }
    defs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(defs)
}

/// Validate a set of agent definitions for consistency
pub fn validate_definitions(defs: &[AgentDefinition]) -> Result<(), AgentDefinitionError> {
    let mut names = HashSet::new();
    for def in defs {
        if def.name.is_empty() {
            return Err(AgentDefinitionError::Validation(
                "agent name cannot be empty".into(),
            ));
        }
        if def.description.is_empty() {
            return Err(AgentDefinitionError::Validation(format!(
                "agent '{}' has empty description",
                def.name
            )));
        }
        if def.prompt.is_empty() {
            return Err(AgentDefinitionError::Validation(format!(
                "agent '{}' has empty prompt",
                def.name
            )));
        }
        if !names.insert(&def.name) {
            return Err(AgentDefinitionError::Validation(format!(
                "duplicate agent name: {}",
                def.name
            )));
        }
    }

    for def in defs {
        for role in &def.spawnable_roles {
            if !names.contains(role) {
                return Err(AgentDefinitionError::Validation(format!(
                    "agent '{}' references unknown spawnable role '{}'",
                    def.name, role
                )));
            }
        }
    }

    Ok(())
}

/// Convert agent definitions to Claude Code `--agents` JSON format
///
/// The format is: `{"name": {"description": "...", "prompt": "..."}}`
pub fn to_claude_agents_json(defs: &[AgentDefinition]) -> Result<String, AgentDefinitionError> {
    let mut map = serde_json::Map::new();
    for def in defs {
        let mut agent = serde_json::Map::new();
        agent.insert(
            "description".into(),
            serde_json::Value::String(def.description.clone()),
        );
        agent.insert(
            "prompt".into(),
            serde_json::Value::String(def.prompt.clone()),
        );
        map.insert(def.name.clone(), serde_json::Value::Object(agent));
    }
    Ok(serde_json::to_string(&map)?)
}

/// Interpolate template variables in a prompt string.
/// Replaces `{{repo}}`, `{{session_id}}`, `{{session_dir}}`, `{{mcp_socket}}`.
pub fn interpolate_prompt(prompt: &str, ctx: &TemplateContext) -> String {
    prompt
        .replace("{{repo}}", &ctx.repo)
        .replace("{{session_id}}", &ctx.session_id)
        .replace("{{session_dir}}", &ctx.session_dir)
        .replace("{{mcp_socket}}", &ctx.mcp_socket)
}

impl AgentDefinition {
    /// Build CLI arguments for spawning this agent via Claude Code
    pub fn to_cli_args(
        &self,
        all_defs: &[AgentDefinition],
        ctx: &TemplateContext,
        mcp_config_path: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<ClaudeAgentArgs, AgentDefinitionError> {
        let interpolated_prompt = interpolate_prompt(&self.prompt, ctx);

        let mut modified_defs: Vec<AgentDefinition> = all_defs.to_vec();
        for def in &mut modified_defs {
            if def.name == self.name {
                def.prompt = interpolated_prompt.clone();
            }
        }

        let agents_json = to_claude_agents_json(&modified_defs)?;

        Ok(ClaudeAgentArgs {
            agents_json,
            agent_name: self.name.clone(),
            permission_mode: self.permissions.clone(),
            allowed_tools: self.allowed_tools.clone(),
            mcp_config: mcp_config_path.map(String::from),
            session_id: session_id.map(String::from),
            extra_args: self.default_args.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_deserialize_full_definition() {
        let yaml = r#"
name: planner
description: Plans work
prompt: You are a planner for {{repo}}
permissions: bypassPermissions
allowed_tools:
  - Bash
  - Read
spawnable_roles:
  - developer
default_args:
  - "--verbose"
"#;
        let def: AgentDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.name, "planner");
        assert_eq!(def.description, "Plans work");
        assert_eq!(def.prompt, "You are a planner for {{repo}}");
        assert_eq!(def.permissions.as_deref(), Some("bypassPermissions"));
        assert_eq!(def.allowed_tools, vec!["Bash", "Read"]);
        assert_eq!(def.spawnable_roles, vec!["developer"]);
        assert_eq!(def.default_args, vec!["--verbose"]);
    }

    #[test]
    fn test_deserialize_minimal_definition() {
        let yaml = r#"
name: minimal
description: A minimal agent
prompt: Do something
"#;
        let def: AgentDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.name, "minimal");
        assert!(def.permissions.is_none());
        assert!(def.allowed_tools.is_empty());
        assert!(def.spawnable_roles.is_empty());
        assert!(def.default_args.is_empty());
    }

    #[test]
    fn test_load_agent_definition() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.yaml");
        std::fs::write(
            &path,
            "name: test\ndescription: Test agent\nprompt: Hello\n",
        )
        .unwrap();

        let def = load_agent_definition(&path).unwrap();
        assert_eq!(def.name, "test");
        assert_eq!(def.description, "Test agent");
        assert_eq!(def.prompt, "Hello");
    }

    #[test]
    fn test_load_agent_definitions_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("b.yaml"),
            "name: bravo\ndescription: B\nprompt: B prompt\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("a.yml"),
            "name: alpha\ndescription: A\nprompt: A prompt\n",
        )
        .unwrap();

        let defs = load_agent_definitions(dir.path()).unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "alpha");
        assert_eq!(defs[1].name, "bravo");
    }

    #[test]
    fn test_load_skips_non_yaml() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("agent.yaml"),
            "name: agent\ndescription: D\nprompt: P\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("readme.txt"), "not yaml").unwrap();
        std::fs::write(dir.path().join("data.json"), "{}").unwrap();

        let defs = load_agent_definitions(dir.path()).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "agent");
    }

    #[test]
    fn test_validate_valid() {
        let defs = vec![
            AgentDefinition {
                name: "coordinator".into(),
                description: "Coordinates".into(),
                prompt: "Do coordination".into(),
                permissions: None,
                allowed_tools: vec![],
                spawnable_roles: vec!["developer".into()],
                default_args: vec![],
            },
            AgentDefinition {
                name: "developer".into(),
                description: "Develops".into(),
                prompt: "Write code".into(),
                permissions: None,
                allowed_tools: vec![],
                spawnable_roles: vec![],
                default_args: vec![],
            },
        ];
        assert!(validate_definitions(&defs).is_ok());
    }

    #[test]
    fn test_validate_duplicate_names() {
        let defs = vec![
            AgentDefinition {
                name: "agent".into(),
                description: "A".into(),
                prompt: "P".into(),
                permissions: None,
                allowed_tools: vec![],
                spawnable_roles: vec![],
                default_args: vec![],
            },
            AgentDefinition {
                name: "agent".into(),
                description: "B".into(),
                prompt: "Q".into(),
                permissions: None,
                allowed_tools: vec![],
                spawnable_roles: vec![],
                default_args: vec![],
            },
        ];
        let err = validate_definitions(&defs).unwrap_err();
        assert!(err.to_string().contains("duplicate agent name"));
    }

    #[test]
    fn test_validate_invalid_spawnable_role() {
        let defs = vec![AgentDefinition {
            name: "coordinator".into(),
            description: "Coordinates".into(),
            prompt: "Do things".into(),
            permissions: None,
            allowed_tools: vec![],
            spawnable_roles: vec!["nonexistent".into()],
            default_args: vec![],
        }];
        let err = validate_definitions(&defs).unwrap_err();
        assert!(err.to_string().contains("unknown spawnable role"));
    }

    #[test]
    fn test_validate_empty_name() {
        let defs = vec![AgentDefinition {
            name: "".into(),
            description: "D".into(),
            prompt: "P".into(),
            permissions: None,
            allowed_tools: vec![],
            spawnable_roles: vec![],
            default_args: vec![],
        }];
        let err = validate_definitions(&defs).unwrap_err();
        assert!(err.to_string().contains("name cannot be empty"));
    }

    #[test]
    fn test_to_claude_agents_json() {
        let defs = vec![AgentDefinition {
            name: "planner".into(),
            description: "Plans work".into(),
            prompt: "You are a planner".into(),
            permissions: None,
            allowed_tools: vec![],
            spawnable_roles: vec![],
            default_args: vec![],
        }];
        let json = to_claude_agents_json(&defs).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["planner"]["description"], "Plans work");
        assert_eq!(parsed["planner"]["prompt"], "You are a planner");
    }

    #[test]
    fn test_interpolate_prompt() {
        let ctx = TemplateContext {
            repo: "owner/repo".into(),
            session_id: "sess-123".into(),
            session_dir: "/tmp/sessions/sess-123".into(),
            mcp_socket: "/tmp/mcp.sock".into(),
        };
        let result = interpolate_prompt(
            "Working on {{repo}} in {{session_dir}} with socket {{mcp_socket}} ({{session_id}})",
            &ctx,
        );
        assert_eq!(
            result,
            "Working on owner/repo in /tmp/sessions/sess-123 with socket /tmp/mcp.sock (sess-123)"
        );
    }

    #[test]
    fn test_interpolate_no_variables() {
        let ctx = TemplateContext {
            repo: "r".into(),
            session_id: "s".into(),
            session_dir: "d".into(),
            mcp_socket: "m".into(),
        };
        let input = "No template variables here";
        assert_eq!(interpolate_prompt(input, &ctx), input);
    }

    #[test]
    fn test_to_cli_args() {
        let defs = vec![
            AgentDefinition {
                name: "planner".into(),
                description: "Plans".into(),
                prompt: "Plan for {{repo}}".into(),
                permissions: Some("bypassPermissions".into()),
                allowed_tools: vec!["Bash".into()],
                spawnable_roles: vec![],
                default_args: vec!["--verbose".into()],
            },
            AgentDefinition {
                name: "dev".into(),
                description: "Develops".into(),
                prompt: "Code for {{repo}}".into(),
                permissions: None,
                allowed_tools: vec![],
                spawnable_roles: vec![],
                default_args: vec![],
            },
        ];
        let ctx = TemplateContext {
            repo: "my/repo".into(),
            session_id: "s1".into(),
            session_dir: "/tmp/s1".into(),
            mcp_socket: "/tmp/mcp.sock".into(),
        };

        let args = defs[0]
            .to_cli_args(&defs, &ctx, Some("/tmp/mcp.json"), Some("s1"))
            .unwrap();

        assert_eq!(args.agent_name, "planner");
        assert_eq!(args.permission_mode.as_deref(), Some("bypassPermissions"));
        assert_eq!(args.allowed_tools, vec!["Bash"]);
        assert_eq!(args.mcp_config.as_deref(), Some("/tmp/mcp.json"));
        assert_eq!(args.session_id.as_deref(), Some("s1"));
        assert_eq!(args.extra_args, vec!["--verbose"]);

        // Verify the JSON contains the interpolated prompt for planner
        let parsed: serde_json::Value = serde_json::from_str(&args.agents_json).unwrap();
        assert_eq!(parsed["planner"]["prompt"], "Plan for my/repo");
        // dev prompt should remain uninterpolated
        assert_eq!(parsed["dev"]["prompt"], "Code for {{repo}}");
    }
}
