// Copyright 2022 Singularity Data
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;

use pgwire::pg_response::PgResponse;
use risingwave_common::error::{ErrorCode, Result};
use risingwave_sqlparser::ast::{DropStatement, ObjectName, ObjectType, Statement};

use crate::session::{OptimizerContext, SessionImpl};

pub mod create_mv;
mod create_source;
pub mod create_table;
pub mod drop_mv;
pub mod drop_table;
mod explain;
mod flush;
#[allow(dead_code)]
mod query;
mod query_single;
mod show_source;
pub mod util;

pub(super) async fn handle(session: Arc<SessionImpl>, stmt: Statement) -> Result<PgResponse> {
    let context = OptimizerContext::new(session.clone());
    match stmt {
        Statement::Explain {
            statement, verbose, ..
        } => explain::handle_explain(context, *statement, verbose),
        Statement::CreateSource(stmt) => create_source::handle_create_source(context, stmt).await,
        Statement::CreateTable { name, columns, .. } => {
            create_table::handle_create_table(context, name, columns).await
        }
        // Since table and source both have source info, use show_source handler can get column info
        Statement::ShowTable { name } => show_source::handle_show_source(context, name).await,
        Statement::ShowSource { name } => show_source::handle_show_source(context, name).await,
        Statement::Drop(DropStatement {
            object_type: ObjectType::Table,
            name,
            ..
        }) => {
            let table_object_name = ObjectName(vec![name]);
            drop_table::handle_drop_table(context, table_object_name).await
        }
        Statement::Drop(DropStatement {
            object_type: ObjectType::MaterializedView,
            name,
            ..
        }) => {
            let table_object_name = ObjectName(vec![name]);
            drop_mv::handle_drop_mv(context, table_object_name).await
        }
        Statement::Query(_) => query_single::handle_query_single(context, stmt).await,
        Statement::Insert { .. } | Statement::Delete { .. } => {
            query_single::handle_query_single(context, stmt).await
        }
        Statement::CreateView {
            materialized: true,
            or_replace: false,
            name,
            query,
            ..
        } => create_mv::handle_create_mv(context, name, query).await,
        Statement::Flush => flush::handle_flush(context).await,
        _ => Err(ErrorCode::NotImplementedError(format!("Unhandled ast: {:?}", stmt)).into()),
    }
}