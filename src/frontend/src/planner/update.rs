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

use risingwave_common::error::{ErrorCode, Result};

use super::Planner;
use crate::binder::BoundUpdate;
use crate::optimizer::PlanRoot;

impl Planner {
    pub(super) fn plan_update(&mut self, update: BoundUpdate) -> Result<PlanRoot> {
        println!("bound update: {:#?}", update);

        Err(ErrorCode::NotImplemented("Planning `UPDATE`".to_owned(), 784.into()).into())
    }
}
