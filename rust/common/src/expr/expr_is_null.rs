use std::sync::Arc;

use crate::array::{ArrayBuilder, ArrayImpl, ArrayRef, BoolArrayBuilder, DataChunk};
use crate::error::Result;
use crate::expr::{BoxedExpression, Expression};
use crate::types::{BoolType, DataType, DataTypeRef};

#[derive(Debug)]
pub struct IsNullExpression {
    child: BoxedExpression,
    return_type: DataTypeRef,
}

#[derive(Debug)]
pub struct IsNotNullExpression {
    child: BoxedExpression,
    return_type: DataTypeRef,
}

impl IsNullExpression {
    pub(crate) fn new(child: BoxedExpression) -> Self {
        Self {
            child,
            return_type: BoolType::create(false),
        }
    }
}

impl IsNotNullExpression {
    pub(crate) fn new(child: BoxedExpression) -> Self {
        Self {
            child,
            return_type: BoolType::create(false),
        }
    }
}

impl Expression for IsNullExpression {
    fn return_type(&self) -> &dyn DataType {
        self.return_type.as_ref()
    }

    fn return_type_ref(&self) -> DataTypeRef {
        self.return_type.clone()
    }

    fn eval(&mut self, input: &DataChunk) -> Result<ArrayRef> {
        let mut builder = BoolArrayBuilder::new(input.cardinality())?;
        self.child
            .eval(input)?
            .null_bitmap()
            .iter()
            .try_for_each(|b| builder.append(Some(!b)))?;

        Ok(Arc::new(ArrayImpl::Bool(builder.finish()?)))
    }
}

impl Expression for IsNotNullExpression {
    fn return_type(&self) -> &dyn DataType {
        self.return_type.as_ref()
    }

    fn return_type_ref(&self) -> DataTypeRef {
        self.return_type.clone()
    }

    fn eval(&mut self, input: &DataChunk) -> Result<ArrayRef> {
        let mut builder = BoolArrayBuilder::new(input.cardinality())?;
        self.child
            .eval(input)?
            .null_bitmap()
            .iter()
            .try_for_each(|b| builder.append(Some(b)))?;

        Ok(Arc::new(ArrayImpl::Bool(builder.finish()?)))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use crate::array::column::Column;
    use crate::array::{ArrayBuilder, ArrayImpl, DataChunk, DecimalArrayBuilder};
    use crate::error::Result;
    use crate::expr::expr_is_null::{IsNotNullExpression, IsNullExpression};
    use crate::expr::{BoxedExpression, InputRefExpression};
    use crate::types::{Decimal, DecimalType};

    fn do_test(mut expr: BoxedExpression, expected_result: Vec<bool>) -> Result<()> {
        let input_array = {
            let mut builder = DecimalArrayBuilder::new(3)?;
            builder.append(Some(Decimal::from_str("0.1").unwrap()))?;
            builder.append(Some(Decimal::from_str("-0.1").unwrap()))?;
            builder.append(None)?;
            builder.finish()?
        };

        let input_chunk = DataChunk::builder()
            .columns(vec![Column::new(Arc::new(ArrayImpl::Decimal(input_array)))])
            .build();
        let result_array = expr.eval(&input_chunk).unwrap();
        assert_eq!(3, result_array.len());
        for (i, v) in expected_result.iter().enumerate() {
            assert_eq!(
                *v,
                bool::try_from(result_array.value_at(i).unwrap()).unwrap()
            );
        }
        Ok(())
    }

    #[test]
    fn test_is_null() -> Result<()> {
        let decimal_type = DecimalType::create(true, 10, 2)?;
        let expr =
            IsNullExpression::new(Box::new(InputRefExpression::new(decimal_type.clone(), 0)));
        do_test(Box::new(expr), vec![false, false, true]).unwrap();
        Ok(())
    }

    #[test]
    fn test_is_not_null() -> Result<()> {
        let decimal_type = DecimalType::create(true, 10, 2)?;
        let expr =
            IsNotNullExpression::new(Box::new(InputRefExpression::new(decimal_type.clone(), 0)));
        do_test(Box::new(expr), vec![true, true, false]).unwrap();
        Ok(())
    }
}