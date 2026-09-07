//! The single zen `CustomNodeAdapter` implementation. Dispatches a JDM
//! `CustomNode` to the matching `CanvasProcessor` via the registry. Unknown kind
//! and bad config map to a typed `NodeError` — NEVER `unwrap`/panic, NEVER
//! silently default a branch.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use zen_engine::nodes::custom::{CustomNodeAdapter, CustomNodeRequest};
use zen_engine::nodes::{NodeError, NodeResponse, NodeResult};

use crate::context::EvaluationContext;
use crate::processors::{ProcessorError, ProcessorRegistry};

#[derive(Debug)]
pub struct CanvasNodeAdapter {
    pub registry: Arc<ProcessorRegistry>,
    pub ctx: Arc<EvaluationContext>,
}

impl CustomNodeAdapter for CanvasNodeAdapter {
    fn handle(&self, request: CustomNodeRequest) -> Pin<Box<dyn Future<Output = NodeResult> + '_>> {
        Box::pin(async move {
            let kind = request.node.kind.as_ref();
            let node_id = request.node.id.clone();
            let proc = self.registry.get(kind).ok_or_else(|| NodeError {
                node_id: node_id.clone(),
                trace: None,
                source: Box::new(ProcessorError::UnknownKind(kind.to_string())),
            })?;
            let outcome = proc
                .evaluate(request.node.config.as_ref(), &self.ctx)
                .map_err(|e| NodeError {
                    node_id,
                    trace: None,
                    source: Box::new(e),
                })?;
            Ok(NodeResponse {
                output: outcome.into_variable(),
                trace_data: None,
            })
        })
    }
}
