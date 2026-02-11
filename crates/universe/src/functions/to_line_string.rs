use std::any::Any;
use std::sync::{Arc, LazyLock, OnceLock};

use arrow_schema::{DataType, Field, FieldRef};
use datafusion::common::{Result, exec_datafusion_err, exec_err};
use datafusion::logical_expr::scalar_doc_sections::DOC_SECTION_OTHER;
use datafusion::logical_expr::{
    ColumnarValue, Documentation, ReturnFieldArgs, ScalarFunctionArgs, ScalarUDFImpl, Signature,
    Volatility,
};
use geoarrow::array::from_arrow_array;
use geoarrow_array::GeoArrowArray;
use geoarrow_schema::{CoordType, Dimension, LineStringType};

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct ToLineString;

impl ToLineString {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ToLineString {
    fn default() -> Self {
        Self::new()
    }
}

// static ALIASES: LazyLock<Vec<String>> = LazyLock::new(|| vec!["st_length2d".to_string()]);
static DOCUMENTATION: OnceLock<Documentation> = OnceLock::new();
static RAW_LINE_STRING_TYPE_INPUT: LazyLock<Signature> = LazyLock::new(|| {
    Signature::exact(
        vec![DataType::new_list(
            DataType::Struct(
                vec![
                    Field::new("x", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                ]
                .into(),
            ),
            false,
        )],
        Volatility::Immutable,
    )
});

impl ScalarUDFImpl for ToLineString {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        "to_line_string"
    }

    fn aliases(&self) -> &[String] {
        &[]
    }

    fn signature(&self) -> &Signature {
        &RAW_LINE_STRING_TYPE_INPUT
    }

    fn return_type(&self, _arg_types: &[DataType]) -> Result<DataType> {
        Ok(LineStringType::new(Dimension::XY, Default::default())
            .with_coord_type(CoordType::Separated)
            .data_type())
    }

    fn return_field_from_args(&self, args: ReturnFieldArgs) -> Result<FieldRef> {
        if args.arg_fields.len() != 1 {
            return exec_err!("Expected exactly 1 argument, got {}", args.arg_fields.len());
        }
        let field = &args.arg_fields[0];
        let field = LineStringType::new(Dimension::XY, Default::default())
            .with_coord_type(CoordType::Separated)
            .to_field(field.name(), field.is_nullable());
        Ok(Arc::new(field))
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        to_line_string_impl(args)
    }

    fn documentation(&self) -> Option<&Documentation> {
        Some(DOCUMENTATION.get_or_init(|| {
            Documentation::builder(
                DOC_SECTION_OTHER,
                "Anootate LinString-shaped data with arrow extension metadata.",
                "to_line_string(raw)",
            )
            .with_argument("geom", "geometry")
            .build()
        }))
    }
}

fn to_line_string_impl(args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    let arrays = ColumnarValue::values_to_arrays(&args.args)?;
    if arrays.len() != 1 {
        return exec_err!("Expected exactly 1 arguments, got {}", arrays.len());
    }
    let field = &args.arg_fields[0];
    let field = LineStringType::new(Dimension::XY, Default::default())
        .with_coord_type(CoordType::Separated)
        .to_field(field.name(), field.is_nullable());
    let geo_array =
        from_arrow_array(&arrays[0], &field).map_err(|e| exec_datafusion_err!("{e}"))?;
    Ok(ColumnarValue::Array(geo_array.into_array_ref()))
}
