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

use std::fmt;

use fixedbitset::FixedBitSet;
use itertools::Itertools;
use risingwave_common::catalog::Field;
use risingwave_common::error::Result;
use risingwave_common::types::{DataType, IntervalUnit};

use super::{
    BatchHopWindow, ColPrunable, LogicalProject, PlanBase, PlanRef, PlanTreeNodeUnary,
    StreamHopWindow, ToBatch, ToStream,
};
use crate::expr::{InputRef, InputRefDisplay};
use crate::utils::ColIndexMapping;

/// `LogicalHopWindow` implements Hop Table Function.
#[derive(Debug, Clone)]
pub struct LogicalHopWindow {
    pub base: PlanBase,
    input: PlanRef,
    pub(super) time_col: InputRef,
    pub(super) window_slide: IntervalUnit,
    pub(super) window_size: IntervalUnit,
}

impl LogicalHopWindow {
    fn new(
        input: PlanRef,
        time_col: InputRef,
        window_slide: IntervalUnit,
        window_size: IntervalUnit,
    ) -> Self {
        let ctx = input.ctx();
        let schema = input
            .schema()
            .clone()
            .into_fields()
            .into_iter()
            .chain([
                Field::with_name(DataType::Timestamp, "window_start"),
                Field::with_name(DataType::Timestamp, "window_end"),
            ])
            .collect();
        let mut pk_indices = input.pk_indices().to_vec();
        pk_indices.push(input.schema().len());
        let base = PlanBase::new_logical(ctx, schema, pk_indices);
        LogicalHopWindow {
            base,
            input,
            time_col,
            window_slide,
            window_size,
        }
    }

    /// the function will check if the cond is bool expression
    pub fn create(
        input: PlanRef,
        time_col: InputRef,
        window_slide: IntervalUnit,
        window_size: IntervalUnit,
    ) -> PlanRef {
        Self::new(input, time_col, window_slide, window_size).into()
    }

    pub fn window_start_col_idx(&self) -> usize {
        self.schema().len() - 2
    }

    pub fn window_end_col_idx(&self) -> usize {
        self.schema().len() - 1
    }

    pub fn o2i_col_mapping(&self) -> ColIndexMapping {
        ColIndexMapping::identity_or_none(self.schema().len(), self.input.schema().len())
    }

    pub fn i2o_col_mapping(&self) -> ColIndexMapping {
        ColIndexMapping::identity_or_none(self.input.schema().len(), self.schema().len())
    }

    pub fn fmt_with_name(&self, f: &mut fmt::Formatter, name: &str) -> fmt::Result {
        write!(
            f,
            "{} {{ time_col: {} slide: {} size: {} }}",
            name,
            InputRefDisplay(self.time_col.index),
            self.window_slide,
            self.window_size
        )
    }
}

impl PlanTreeNodeUnary for LogicalHopWindow {
    fn input(&self) -> PlanRef {
        self.input.clone()
    }

    fn clone_with_input(&self, input: PlanRef) -> Self {
        Self::new(
            input,
            self.time_col.clone(),
            self.window_slide,
            self.window_size,
        )
    }

    #[must_use]
    fn rewrite_with_input(
        &self,
        input: PlanRef,
        input_col_change: ColIndexMapping,
    ) -> (Self, ColIndexMapping) {
        let mut time_col = self.time_col.clone();
        time_col.index = input_col_change.map(time_col.index);
        let new_hop = Self::new(input.clone(), time_col, self.window_slide, self.window_size);

        let (mut mapping, new_input_col_num) = input_col_change.into_parts();
        assert_eq!(new_input_col_num, input.schema().len());
        assert_eq!(new_input_col_num + 2, new_hop.schema().len());
        mapping.push(Some(new_input_col_num));
        mapping.push(Some(new_input_col_num + 1));

        (new_hop, ColIndexMapping::new(mapping))
    }
}

impl_plan_tree_node_for_unary! {LogicalHopWindow}

impl fmt::Display for LogicalHopWindow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_with_name(f, "LogicalHopWindow")
    }
}

impl ColPrunable for LogicalHopWindow {
    fn prune_col(&self, required_cols: &[usize]) -> PlanRef {
        let require_win_start = required_cols.contains(&self.window_start_col_idx());
        let require_win_end = required_cols.contains(&self.window_end_col_idx());
        let o2i = self.o2i_col_mapping();
        let input_required_cols = {
            let mut tmp = FixedBitSet::with_capacity(self.schema().len());
            tmp.extend(required_cols.iter().copied());
            tmp = o2i.rewrite_bitset(&tmp);
            // LogicalHopWindow should keep all required cols from upstream,
            // as well as its own time_col.
            tmp.put(self.time_col.index());
            tmp.ones().collect_vec()
        };
        let input = self.input.prune_col(&input_required_cols);
        let input_change = ColIndexMapping::with_remaining_columns(
            &input_required_cols,
            self.input().schema().len(),
        );
        let (new_hop, _) = self.rewrite_with_input(input, input_change);
        let required_output_cols = {
            // output cols = { cols required by upstream from input node } ∪ { additional window
            // cols }

            // `required_output_cols` indicates whether a column is needed or not
            // get indices of columns which are required by upstream in `new_hop`
            let mut required_output_cols = {
                // map the indices from output to input
                let input_required_cols = required_cols
                    .iter()
                    .filter_map(|&idx| o2i.try_map(idx))
                    .collect_vec();
                // this mapping will only keeps required input columns
                let mapping = ColIndexMapping::with_remaining_columns(
                    &input_required_cols,
                    self.input().schema().len(),
                );
                input_required_cols
                    .iter()
                    .filter_map(|&idx| mapping.try_map(idx))
                    .collect_vec()
            };

            let win_start_index = new_hop.schema().len() - 2;
            let win_end_index = new_hop.schema().len() - 1;
            if require_win_start {
                required_output_cols.push(win_start_index);
            }
            if require_win_end {
                required_output_cols.push(win_end_index);
            }
            required_output_cols
        };
        LogicalProject::with_mapping(
            new_hop.into(),
            ColIndexMapping::with_remaining_columns(&required_output_cols, self.schema().len()),
        )
        .into()
    }
}

impl ToBatch for LogicalHopWindow {
    fn to_batch(&self) -> Result<PlanRef> {
        let new_input = self.input().to_batch()?;
        let new_logical = self.clone_with_input(new_input);
        Ok(BatchHopWindow::new(new_logical).into())
    }
}

impl ToStream for LogicalHopWindow {
    fn to_stream(&self) -> Result<PlanRef> {
        let new_input = self.input().to_stream()?;
        let new_logical = self.clone_with_input(new_input);
        Ok(StreamHopWindow::new(new_logical).into())
    }

    fn logical_rewrite_for_stream(&self) -> Result<(PlanRef, ColIndexMapping)> {
        let (input, input_col_change) = self.input.logical_rewrite_for_stream()?;
        let (hop, out_col_change) = self.rewrite_with_input(input, input_col_change);
        Ok((hop.into(), out_col_change))
    }
}
