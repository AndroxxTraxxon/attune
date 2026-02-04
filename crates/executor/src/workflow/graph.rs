//! Task Graph Builder
//!
//! This module builds executable task graphs from workflow definitions.
//! Workflows are directed graphs where tasks are nodes and transitions are edges.
//! Execution follows transitions from completed tasks, naturally supporting cycles.

use attune_common::workflow::{Task, TaskType, WorkflowDefinition};
use std::collections::{HashMap, HashSet};

/// Result type for graph operations
pub type GraphResult<T> = Result<T, GraphError>;

/// Errors that can occur during graph building
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("Invalid task reference: {0}")]
    InvalidTaskReference(String),

    #[error("Graph building error: {0}")]
    BuildError(String),
}

/// Executable task graph
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskGraph {
    /// All nodes in the graph
    pub nodes: HashMap<String, TaskNode>,

    /// Entry points (tasks with no inbound edges)
    pub entry_points: Vec<String>,

    /// Inbound edges map (task -> tasks that can transition to it)
    pub inbound_edges: HashMap<String, HashSet<String>>,

    /// Outbound edges map (task -> tasks it can transition to)
    pub outbound_edges: HashMap<String, HashSet<String>>,
}

/// A node in the task graph
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskNode {
    /// Task name
    pub name: String,

    /// Task type
    pub task_type: TaskType,

    /// Action reference (for action tasks)
    pub action: Option<String>,

    /// Input template
    pub input: serde_json::Value,

    /// Conditional execution
    pub when: Option<String>,

    /// With-items iteration
    pub with_items: Option<String>,

    /// Batch size for iterations
    pub batch_size: Option<usize>,

    /// Concurrency limit
    pub concurrency: Option<usize>,

    /// Variable publishing directives
    pub publish: Vec<String>,

    /// Retry configuration
    pub retry: Option<RetryConfig>,

    /// Timeout in seconds
    pub timeout: Option<u32>,

    /// Transitions
    pub transitions: TaskTransitions,

    /// Sub-tasks (for parallel tasks)
    pub sub_tasks: Option<Vec<TaskNode>>,

    /// Inbound tasks (computed - tasks that can transition to this one)
    pub inbound_tasks: HashSet<String>,

    /// Join count (if specified, wait for N inbound tasks to complete)
    pub join: Option<usize>,
}

/// Task transitions
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TaskTransitions {
    pub on_success: Option<String>,
    pub on_failure: Option<String>,
    pub on_complete: Option<String>,
    pub on_timeout: Option<String>,
    pub decision: Vec<DecisionBranch>,
}

/// Decision branch
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionBranch {
    pub when: Option<String>,
    pub next: String,
    pub default: bool,
}

/// Retry configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryConfig {
    pub count: u32,
    pub delay: u32,
    pub backoff: BackoffStrategy,
    pub max_delay: Option<u32>,
    pub on_error: Option<String>,
}

/// Backoff strategy
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BackoffStrategy {
    Constant,
    Linear,
    Exponential,
}

impl TaskGraph {
    /// Create a graph from a workflow definition
    pub fn from_workflow(workflow: &WorkflowDefinition) -> GraphResult<Self> {
        let mut builder = GraphBuilder::new();

        for task in &workflow.tasks {
            builder.add_task(task)?;
        }

        // Build the graph
        let builder = builder.build()?;
        Ok(builder.into())
    }

    /// Get a task node by name
    pub fn get_task(&self, name: &str) -> Option<&TaskNode> {
        self.nodes.get(name)
    }

    /// Get all tasks that can transition into the given task (inbound edges)
    pub fn get_inbound_tasks(&self, task_name: &str) -> Vec<String> {
        self.inbound_edges
            .get(task_name)
            .map(|tasks| tasks.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the next tasks to execute after a task completes.
    /// Evaluates transitions based on task status.
    ///
    /// # Arguments
    /// * `task_name` - The name of the task that completed
    /// * `success` - Whether the task succeeded
    ///
    /// # Returns
    /// A vector of task names to schedule next
    pub fn next_tasks(&self, task_name: &str, success: bool) -> Vec<String> {
        let mut next = Vec::new();

        if let Some(node) = self.nodes.get(task_name) {
            // Check explicit transitions based on task status
            if success {
                if let Some(ref next_task) = node.transitions.on_success {
                    next.push(next_task.clone());
                }
            } else if let Some(ref next_task) = node.transitions.on_failure {
                next.push(next_task.clone());
            }

            // on_complete runs regardless of success/failure
            if let Some(ref next_task) = node.transitions.on_complete {
                next.push(next_task.clone());
            }

            // Decision branches (evaluated separately in coordinator with context)
            // We don't evaluate them here since they need runtime context
        }

        next
    }
}

/// Graph builder helper
struct GraphBuilder {
    nodes: HashMap<String, TaskNode>,
    inbound_edges: HashMap<String, HashSet<String>>,
}

impl GraphBuilder {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            inbound_edges: HashMap::new(),
        }
    }

    fn add_task(&mut self, task: &Task) -> GraphResult<()> {
        let node = self.task_to_node(task)?;
        self.nodes.insert(task.name.clone(), node);
        Ok(())
    }

    fn task_to_node(&self, task: &Task) -> GraphResult<TaskNode> {
        let publish = extract_publish_vars(&task.publish);

        let retry = task.retry.as_ref().map(|r| RetryConfig {
            count: r.count,
            delay: r.delay,
            backoff: match r.backoff {
                attune_common::workflow::BackoffStrategy::Constant => BackoffStrategy::Constant,
                attune_common::workflow::BackoffStrategy::Linear => BackoffStrategy::Linear,
                attune_common::workflow::BackoffStrategy::Exponential => {
                    BackoffStrategy::Exponential
                }
            },
            max_delay: r.max_delay,
            on_error: r.on_error.clone(),
        });

        let transitions = TaskTransitions {
            on_success: task.on_success.clone(),
            on_failure: task.on_failure.clone(),
            on_complete: task.on_complete.clone(),
            on_timeout: task.on_timeout.clone(),
            decision: task
                .decision
                .iter()
                .map(|d| DecisionBranch {
                    when: d.when.clone(),
                    next: d.next.clone(),
                    default: d.default,
                })
                .collect(),
        };

        let sub_tasks = if let Some(ref tasks) = task.tasks {
            let mut sub_nodes = Vec::new();
            for subtask in tasks {
                sub_nodes.push(self.task_to_node(subtask)?);
            }
            Some(sub_nodes)
        } else {
            None
        };

        Ok(TaskNode {
            name: task.name.clone(),
            task_type: task.r#type.clone(),
            action: task.action.clone(),
            input: serde_json::to_value(&task.input).unwrap_or(serde_json::json!({})),
            when: task.when.clone(),
            with_items: task.with_items.clone(),
            batch_size: task.batch_size,
            concurrency: task.concurrency,
            publish,
            retry,
            timeout: task.timeout,
            transitions,
            sub_tasks,
            inbound_tasks: HashSet::new(),
            join: task.join,
        })
    }

    fn build(mut self) -> GraphResult<Self> {
        // Compute inbound edges from transitions
        self.compute_inbound_edges()?;

        Ok(self)
    }

    fn compute_inbound_edges(&mut self) -> GraphResult<()> {
        let node_names: Vec<String> = self.nodes.keys().cloned().collect();

        for node_name in &node_names {
            if let Some(node) = self.nodes.get(node_name) {
                // Collect all tasks this task can transition to
                let successors = vec![
                    node.transitions.on_success.as_ref(),
                    node.transitions.on_failure.as_ref(),
                    node.transitions.on_complete.as_ref(),
                    node.transitions.on_timeout.as_ref(),
                ];

                // For each successor, record this task as an inbound edge
                for successor in successors.into_iter().flatten() {
                    if !self.nodes.contains_key(successor) {
                        return Err(GraphError::InvalidTaskReference(format!(
                            "Task '{}' references non-existent task '{}'",
                            node_name, successor
                        )));
                    }

                    self.inbound_edges
                        .entry(successor.clone())
                        .or_insert_with(HashSet::new)
                        .insert(node_name.clone());
                }

                // Add decision branch edges
                for branch in &node.transitions.decision {
                    if !self.nodes.contains_key(&branch.next) {
                        return Err(GraphError::InvalidTaskReference(format!(
                            "Task '{}' decision references non-existent task '{}'",
                            node_name, branch.next
                        )));
                    }

                    self.inbound_edges
                        .entry(branch.next.clone())
                        .or_insert_with(HashSet::new)
                        .insert(node_name.clone());
                }
            }
        }

        // Update node inbound_tasks
        for (name, inbound) in &self.inbound_edges {
            if let Some(node) = self.nodes.get_mut(name) {
                node.inbound_tasks = inbound.clone();
            }
        }

        Ok(())
    }
}

impl From<GraphBuilder> for TaskGraph {
    fn from(builder: GraphBuilder) -> Self {
        // Entry points are tasks with no inbound edges
        let entry_points: Vec<String> = builder
            .nodes
            .keys()
            .filter(|name| {
                builder
                    .inbound_edges
                    .get(*name)
                    .map(|edges| edges.is_empty())
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        // Build outbound edges map (reverse of inbound)
        let mut outbound_edges: HashMap<String, HashSet<String>> = HashMap::new();
        for (task, inbound) in &builder.inbound_edges {
            for source in inbound {
                outbound_edges
                    .entry(source.clone())
                    .or_insert_with(HashSet::new)
                    .insert(task.clone());
            }
        }

        TaskGraph {
            nodes: builder.nodes,
            entry_points,
            inbound_edges: builder.inbound_edges,
            outbound_edges,
        }
    }
}

/// Extract variable names from publish directives
fn extract_publish_vars(publish: &[attune_common::workflow::PublishDirective]) -> Vec<String> {
    use attune_common::workflow::PublishDirective;

    let mut vars = Vec::new();
    for directive in publish {
        match directive {
            PublishDirective::Simple(map) => {
                vars.extend(map.keys().cloned());
            }
            PublishDirective::Key(key) => {
                vars.push(key.clone());
            }
        }
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;
    use attune_common::workflow;

    #[test]
    fn test_simple_sequential_graph() {
        let yaml = r#"
ref: test.sequential
label: Sequential Workflow
version: 1.0.0
tasks:
  - name: task1
    action: core.echo
    on_success: task2
  - name: task2
    action: core.echo
    on_success: task3
  - name: task3
    action: core.echo
"#;

        let workflow = workflow::parse_workflow_yaml(yaml).unwrap();
        let graph = TaskGraph::from_workflow(&workflow).unwrap();

        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.entry_points.len(), 1);
        assert_eq!(graph.entry_points[0], "task1");

        // Check inbound edges
        assert!(graph
            .inbound_edges
            .get("task1")
            .map(|e| e.is_empty())
            .unwrap_or(true));
        assert_eq!(graph.inbound_edges["task2"].len(), 1);
        assert!(graph.inbound_edges["task2"].contains("task1"));
        assert_eq!(graph.inbound_edges["task3"].len(), 1);
        assert!(graph.inbound_edges["task3"].contains("task2"));

        // Check transitions
        let next = graph.next_tasks("task1", true);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0], "task2");

        let next = graph.next_tasks("task2", true);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0], "task3");
    }

    #[test]
    fn test_parallel_entry_points() {
        let yaml = r#"
ref: test.parallel_start
label: Parallel Start
version: 1.0.0
tasks:
  - name: task1
    action: core.echo
    on_success: final
  - name: task2
    action: core.echo
    on_success: final
  - name: final
    action: core.complete
"#;

        let workflow = workflow::parse_workflow_yaml(yaml).unwrap();
        let graph = TaskGraph::from_workflow(&workflow).unwrap();

        assert_eq!(graph.entry_points.len(), 2);
        assert!(graph.entry_points.contains(&"task1".to_string()));
        assert!(graph.entry_points.contains(&"task2".to_string()));

        // final task should have both as inbound edges
        assert_eq!(graph.inbound_edges["final"].len(), 2);
        assert!(graph.inbound_edges["final"].contains("task1"));
        assert!(graph.inbound_edges["final"].contains("task2"));
    }

    #[test]
    fn test_transitions() {
        let yaml = r#"
ref: test.transitions
label: Transition Test
version: 1.0.0
tasks:
  - name: task1
    action: core.echo
    on_success: task2
  - name: task2
    action: core.echo
    on_success: task3
  - name: task3
    action: core.echo
"#;

        let workflow = workflow::parse_workflow_yaml(yaml).unwrap();
        let graph = TaskGraph::from_workflow(&workflow).unwrap();

        // Test next_tasks follows transitions
        let next = graph.next_tasks("task1", true);
        assert_eq!(next, vec!["task2"]);

        let next = graph.next_tasks("task2", true);
        assert_eq!(next, vec!["task3"]);

        // task3 has no transitions
        let next = graph.next_tasks("task3", true);
        assert!(next.is_empty());
    }

    #[test]
    fn test_cycle_support() {
        let yaml = r#"
ref: test.cycle
label: Cycle Test
version: 1.0.0
tasks:
  - name: check
    action: core.check
    on_success: process
    on_failure: check
  - name: process
    action: core.process
"#;

        let workflow = workflow::parse_workflow_yaml(yaml).unwrap();
        // Should not error on cycles
        let graph = TaskGraph::from_workflow(&workflow).unwrap();

        // Note: check has a self-reference (check -> check on failure)
        // So it has an inbound edge and is not an entry point
        // process also has an inbound edge (check -> process on success)
        // Therefore, there are no entry points in this workflow
        assert_eq!(graph.entry_points.len(), 0);

        // check can transition to itself on failure (cycle)
        let next = graph.next_tasks("check", false);
        assert_eq!(next, vec!["check"]);

        // check transitions to process on success
        let next = graph.next_tasks("check", true);
        assert_eq!(next, vec!["process"]);
    }

    #[test]
    fn test_inbound_tasks() {
        let yaml = r#"
ref: test.inbound
label: Inbound Test
version: 1.0.0
tasks:
  - name: task1
    action: core.echo
    on_success: final
  - name: task2
    action: core.echo
    on_success: final
  - name: final
    action: core.complete
"#;

        let workflow = workflow::parse_workflow_yaml(yaml).unwrap();
        let graph = TaskGraph::from_workflow(&workflow).unwrap();

        let inbound = graph.get_inbound_tasks("final");
        assert_eq!(inbound.len(), 2);
        assert!(inbound.contains(&"task1".to_string()));
        assert!(inbound.contains(&"task2".to_string()));

        let inbound = graph.get_inbound_tasks("task1");
        assert_eq!(inbound.len(), 0);
    }
}
