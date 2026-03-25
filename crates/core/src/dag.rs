use std::collections::{HashMap, HashSet};

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use thiserror::Error;

use crate::schema::Workflow;

#[derive(Debug, Error)]
pub enum DagError {
    #[error("workflow contains a cycle involving step '{0}'")]
    Cycle(String),
    #[error("unknown step in dependency: {0}")]
    UnknownStep(String),
}

/// A workflow DAG with step nodes and dependency edges
#[derive(Debug)]
pub struct WorkflowDag {
    graph: DiGraph<String, ()>,
    node_map: HashMap<String, NodeIndex>,
}

impl WorkflowDag {
    pub fn from_workflow(workflow: &Workflow) -> Result<Self, DagError> {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();

        for step in &workflow.steps {
            let idx = graph.add_node(step.name.clone());
            node_map.insert(step.name.clone(), idx);
        }

        for step in &workflow.steps {
            let step_idx = node_map[&step.name];
            for dep in &step.depends_on {
                let dep_idx = *node_map
                    .get(dep)
                    .ok_or_else(|| DagError::UnknownStep(dep.clone()))?;
                graph.add_edge(dep_idx, step_idx, ());
            }
        }

        if let Err(cycle) = toposort(&graph, None) {
            let node_name = &graph[cycle.node_id()];
            return Err(DagError::Cycle(node_name.clone()));
        }

        Ok(Self { graph, node_map })
    }

    pub fn execution_order(&self) -> Result<Vec<String>, DagError> {
        let sorted = toposort(&self.graph, None)
            .map_err(|cycle| DagError::Cycle(self.graph[cycle.node_id()].clone()))?;
        Ok(sorted.iter().map(|idx| self.graph[*idx].clone()).collect())
    }

    pub fn ready_steps(&self, completed: &HashSet<String>) -> Vec<String> {
        let mut ready = Vec::new();
        for (name, &idx) in &self.node_map {
            if completed.contains(name) {
                continue;
            }
            let all_deps_met = self
                .graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .all(|dep_idx| completed.contains(&self.graph[dep_idx]));
            if all_deps_met {
                ready.push(name.clone());
            }
        }
        ready
    }

    pub fn dependencies(&self, step_name: &str) -> Vec<String> {
        if let Some(&idx) = self.node_map.get(step_name) {
            self.graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .map(|dep_idx| self.graph[dep_idx].clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn step_count(&self) -> usize {
        self.graph.node_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Workflow;

    fn make_workflow(steps_yaml: &str) -> Workflow {
        let yaml = format!("name: test\nsteps:\n{}", steps_yaml);
        Workflow::from_yaml(&yaml).unwrap()
    }

    #[test]
    fn test_linear_dag() {
        let wf = make_workflow(
            "  - name: a\n    agent: claude\n  - name: b\n    agent: claude\n    depends_on: [a]\n  - name: c\n    agent: claude\n    depends_on: [b]\n",
        );
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let order = dag.execution_order().unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parallel_dag() {
        let wf = make_workflow(
            "  - name: a\n    agent: claude\n  - name: b\n    agent: claude\n  - name: c\n    agent: claude\n    depends_on: [a, b]\n",
        );
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let ready = dag.ready_steps(&HashSet::new());
        assert!(ready.contains(&"a".to_string()));
        assert!(ready.contains(&"b".to_string()));
        assert!(!ready.contains(&"c".to_string()));
    }

    #[test]
    fn test_ready_after_completion() {
        let wf = make_workflow(
            "  - name: a\n    agent: claude\n  - name: b\n    agent: claude\n    depends_on: [a]\n",
        );
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        let mut completed = HashSet::new();
        completed.insert("a".to_string());
        let ready = dag.ready_steps(&completed);
        assert_eq!(ready, vec!["b"]);
    }

    #[test]
    fn test_cycle_detection() {
        let yaml = "name: test\nsteps:\n  - name: a\n    agent: claude\n    depends_on: [b]\n  - name: b\n    agent: claude\n    depends_on: [a]\n";
        let wf: Workflow = serde_yaml::from_str(yaml).unwrap();
        assert!(WorkflowDag::from_workflow(&wf).is_err());
    }

    #[test]
    fn test_step_count() {
        let wf = make_workflow(
            "  - name: a\n    agent: claude\n  - name: b\n    agent: claude\n  - name: c\n    agent: claude\n",
        );
        let dag = WorkflowDag::from_workflow(&wf).unwrap();
        assert_eq!(dag.step_count(), 3);
    }
}
