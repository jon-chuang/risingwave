use paste::paste;

use super::super::plan_node::*;
use crate::optimizer::plan_node::PlanNodeId;
use crate::session::QueryContextRef;
use crate::{for_batch_plan_nodes, for_logical_plan_nodes, for_stream_plan_nodes};

pub trait WithContext {
    fn ctx(&self) -> QueryContextRef;
}

macro_rules! impl_with_ctx {
    ([], $( { $convention:ident, $name:ident }),*) => {
        $(paste! {
            impl WithContext for [<$convention $name>] {
                fn ctx(&self) -> QueryContextRef {
                    self.base.ctx.clone()
                }
            }
        })*
    }
}
for_batch_plan_nodes! { impl_with_ctx }
for_logical_plan_nodes! { impl_with_ctx }
for_stream_plan_nodes! { impl_with_ctx }

pub trait WithId {
    fn id(&self) -> PlanNodeId;
}

macro_rules! impl_with_id {
    ([], $( { $convention:ident, $name:ident }),*) => {
        $(paste! {
            impl WithId for [<$convention $name>] {
                fn id(&self) -> PlanNodeId {
                    self.base.id
                }
            }
        })*
    }
}
for_batch_plan_nodes! { impl_with_id }
for_logical_plan_nodes! { impl_with_id }
for_stream_plan_nodes! { impl_with_id }