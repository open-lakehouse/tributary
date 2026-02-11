use std::any::Any;
use std::sync::{LazyLock, OnceLock};

use arrow::array::Float64Array;
use arrow_schema::DataType;
use datafusion::common::{Result, exec_datafusion_err, exec_err};
use datafusion::logical_expr::scalar_doc_sections::DOC_SECTION_OTHER;
use datafusion::logical_expr::{
    ColumnarValue, Documentation, ScalarFunctionArgs, ScalarUDFImpl, Signature, TypeSignature,
    Volatility,
};
use geo::{Euclidean, InterpolatableLine};
use geo_traits::to_geo::{ToGeoLine, ToGeoLineString};
use geo_traits::{GeometryTrait, GeometryType};
use geoarrow::array::{PointArray, PointBuilder, from_arrow_array};
use geoarrow_array::{GeoArrowArray, GeoArrowArrayAccessor, downcast_geoarrow_array};
use geoarrow_schema::error::GeoArrowResult;
use geoarrow_schema::{
    BoxType, CoordType, Dimension, GeometryCollectionType, LineStringType, MultiLineStringType,
    MultiPointType, MultiPolygonType, PointType, PolygonType,
};

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct InterpolateLine;

impl InterpolateLine {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for InterpolateLine {
    fn default() -> Self {
        Self::new()
    }
}

// static ALIASES: LazyLock<Vec<String>> = LazyLock::new(|| vec!["st_length2d".to_string()]);
static DOCUMENTATION: OnceLock<Documentation> = OnceLock::new();
static SINGLE_LINE_TYPE_INPUT: LazyLock<Signature> = LazyLock::new(|| {
    let types: Vec<_> = any_line_type()
        .into_iter()
        .map(|dt| TypeSignature::Exact(vec![dt, DataType::Float64]))
        .collect();
    Signature::one_of(types, Volatility::Immutable)
});

impl ScalarUDFImpl for InterpolateLine {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        "st_lineinterpolatepoint"
    }

    fn aliases(&self) -> &[String] {
        &[]
    }

    fn signature(&self) -> &Signature {
        &SINGLE_LINE_TYPE_INPUT
    }

    fn return_type(&self, _arg_types: &[DataType]) -> Result<DataType> {
        Ok(PointType::new(Dimension::XY, Default::default()).data_type())
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        interpolate_line_impl(args)
    }

    fn documentation(&self) -> Option<&Documentation> {
        Some(DOCUMENTATION.get_or_init(|| {
            Documentation::builder(
                DOC_SECTION_OTHER,
                "Interpolate a Point along a Line or LineString.",
                "ST_LineInterpolatePoint(geom, 0.5)",
            )
            .with_argument("geom", "geometry")
            .with_sql_example(
                "SELECT ST_LineInterpolatePoint(ST_GeomFromText('LINESTRING(0 0, 3 4)'), 0.5);",
            )
            .build()
        }))
    }
}

fn interpolate_line_impl(args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    let arrays = ColumnarValue::values_to_arrays(&args.args)?;
    if arrays.len() != 2 {
        return exec_err!("Expected exactly 2 arguments, got {}", arrays.len());
    }
    let field = &args.arg_fields[0];
    let geo_array = from_arrow_array(&arrays[0], field).map_err(|e| exec_datafusion_err!("{e}"))?;
    let frac_arr = arrays[1]
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| exec_datafusion_err!("Expected second argument to be Float64Array"))?;
    let result = interpolate_line(&geo_array, frac_arr).map_err(|e| exec_datafusion_err!("{e}"))?;
    Ok(ColumnarValue::Array(result.into_array_ref()))
}

fn any_line_type() -> Vec<DataType> {
    let mut valid_types = Vec::with_capacity(10);

    for coord_type in [CoordType::Separated, CoordType::Interleaved] {
        for dim in [
            Dimension::XY,
            Dimension::XYZ,
            Dimension::XYM,
            Dimension::XYZM,
        ] {
            valid_types.push(
                LineStringType::new(dim, Default::default())
                    .with_coord_type(coord_type)
                    .data_type(),
            );
        }

        valid_types.push(
            geoarrow_schema::GeometryType::new(Default::default())
                .with_coord_type(coord_type)
                .data_type(),
        );
    }

    // Wkb
    valid_types.push(DataType::Binary);
    valid_types.push(DataType::LargeBinary);
    valid_types.push(DataType::BinaryView);

    // Wkt
    valid_types.push(DataType::Utf8);
    valid_types.push(DataType::LargeUtf8);
    valid_types.push(DataType::Utf8View);

    valid_types
}

fn any_geometry_type() -> Vec<DataType> {
    let expected_capacity = (2 * 4 * 7) + 2 + 4 + 3 + 3;
    let mut valid_types = Vec::with_capacity(expected_capacity);

    for coord_type in [CoordType::Separated, CoordType::Interleaved] {
        for dim in [
            Dimension::XY,
            Dimension::XYZ,
            Dimension::XYM,
            Dimension::XYZM,
        ] {
            valid_types.push(
                PointType::new(dim, Default::default())
                    .with_coord_type(coord_type)
                    .data_type(),
            );
            valid_types.push(
                LineStringType::new(dim, Default::default())
                    .with_coord_type(coord_type)
                    .data_type(),
            );
            valid_types.push(
                PolygonType::new(dim, Default::default())
                    .with_coord_type(coord_type)
                    .data_type(),
            );
            valid_types.push(
                MultiPointType::new(dim, Default::default())
                    .with_coord_type(coord_type)
                    .data_type(),
            );
            valid_types.push(
                MultiLineStringType::new(dim, Default::default())
                    .with_coord_type(coord_type)
                    .data_type(),
            );
            valid_types.push(
                MultiPolygonType::new(dim, Default::default())
                    .with_coord_type(coord_type)
                    .data_type(),
            );
            valid_types.push(
                GeometryCollectionType::new(dim, Default::default())
                    .with_coord_type(coord_type)
                    .data_type(),
            );
        }
    }

    for coord_type in [CoordType::Separated, CoordType::Interleaved] {
        valid_types.push(
            geoarrow_schema::GeometryType::new(Default::default())
                .with_coord_type(coord_type)
                .data_type(),
        );
    }

    for dim in [
        Dimension::XY,
        Dimension::XYZ,
        Dimension::XYM,
        Dimension::XYZM,
    ] {
        valid_types.push(BoxType::new(dim, Default::default()).data_type());
    }

    // Wkb
    valid_types.push(DataType::Binary);
    valid_types.push(DataType::LargeBinary);
    valid_types.push(DataType::BinaryView);

    // Wkt
    valid_types.push(DataType::Utf8);
    valid_types.push(DataType::LargeUtf8);
    valid_types.push(DataType::Utf8View);

    debug_assert_eq!(valid_types.len(), expected_capacity);

    valid_types
}

static ANY_SINGLE_GEOMETRY_TYPE_INPUT: LazyLock<Signature> =
    LazyLock::new(|| Signature::uniform(1, any_geometry_type(), Volatility::Immutable));

pub(crate) fn any_single_geometry_type_input() -> &'static Signature {
    &ANY_SINGLE_GEOMETRY_TYPE_INPUT
}

/// Interpolate a Point along a Line or LineString.
pub fn interpolate_line(
    array: &dyn GeoArrowArray,
    fractions: &Float64Array,
) -> GeoArrowResult<PointArray> {
    downcast_geoarrow_array!(array, _interpolate_line, fractions)
}

pub fn _interpolate_line<'a>(
    array: &'a impl GeoArrowArrayAccessor<'a>,
    fractions: &Float64Array,
) -> GeoArrowResult<PointArray> {
    let mut result = PointBuilder::new(PointType::new(Dimension::XY, Default::default()));
    for (geom, frac) in array.iter().zip(fractions.iter()) {
        if let (Some(geom), Some(frac)) = (geom, frac) {
            match geom?.as_type() {
                GeometryType::Line(l) => {
                    let point = l.to_line().point_at_ratio_from_start(&Euclidean, frac);
                    result.push_point(Some(&point));
                }
                GeometryType::LineString(ls) => {
                    let point = ls
                        .to_line_string()
                        .point_at_ratio_from_start(&Euclidean, frac);
                    result.push_point(point.as_ref());
                }
                _ => result.push_null(),
            }
        } else {
            result.push_null();
        }
    }
    Ok(result.finish())
}

#[cfg(test)]
mod test {

    use arrow::util::pretty::print_batches;
    use datafusion::prelude::SessionContext;
    use geo::{Euclidean, LineString};
    use geoarrow_array::builder::LineStringBuilder;
    use geoarrow_schema::{CoordType, Dimension};
    use geodatafusion::udf::native::io::GeomFromText;

    use super::*;

    #[tokio::test]
    async fn test_linestring_length() {
        let ctx = SessionContext::new();

        ctx.register_udf(InterpolateLine::new().into());
        ctx.register_udf(GeomFromText::new(Default::default()).into());

        let df = ctx
            .sql(
                "SELECT ST_LineInterpolatePoint(ST_GeomFromText('LINESTRING(0 0, 3 4)'), 0.5) as point;",
            )
            .await
            .unwrap();
        let batch = df.collect().await.unwrap();

        print_batches(&batch).unwrap();
    }

    #[test]
    fn test_linestring() {
        let mut linestring_builder = LineStringBuilder::new(
            geoarrow_schema::LineStringType::new(Dimension::XY, Default::default())
                .with_coord_type(CoordType::Separated),
        );
        let linestring_1 = LineString::from(vec![(0.0, 0.0), (3.0, 9.0)]);
        let linestring_2 = LineString::from(vec![(0.0, 0.0), (4.0, 5.0)]);
        let fractions = Float64Array::from(vec![0.5, 0.25]);

        let _ = linestring_builder.push_geometry(Some(&linestring_1));
        let _ = linestring_builder.push_geometry(Some(&linestring_2));
        let linestring_array = linestring_builder.finish();

        let result = interpolate_line(&linestring_array, &fractions).unwrap();

        let p1 = linestring_1
            .point_at_ratio_from_start(&Euclidean, 0.5)
            .unwrap();
        let p2 = linestring_2
            .point_at_ratio_from_start(&Euclidean, 0.25)
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result.value(0).unwrap(), &p1);
        assert_eq!(result.value(1).unwrap(), &p2);
    }
}
